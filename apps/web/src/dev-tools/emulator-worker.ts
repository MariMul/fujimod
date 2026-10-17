import { loadCameraRuntime, type Pyodide, type PyProxy } from '../engine/runtime';
import { EmulatedUsbDevice, emulatedUsb, type UsbSetup } from '../engine/webusb-device';
import { WebUsbCamera, type CameraLink } from './camera';
import { EmulatorCall, type EmulatorRequest, type EmulatorReply } from './emulator-protocol';
import { exportUsbDevice, type SharedDevice } from './usb-sharing';
import { Direction, Layer, Level } from './log';
const worker = self as unknown as DedicatedWorkerGlobalScope;
const camera = new WebUsbCamera();
let py: Pyodide;
let link: CameraLink | undefined;
let usbDevice: EmulatedUsbDevice;
let exported: SharedDevice | undefined;
let transferTimeoutMs: number;
let identity: Record<string, unknown> = {};
let recentInputs: Record<string, unknown>[] = [];
let failures: Record<string, unknown>[] = [];
let trigger: Record<string, unknown> | undefined;
type TransferStep = { state: 'pending' } | { state: 'read' } | { state: 'written'; bytes: number };
function send(reply: EmulatorReply) {
  const checkpoint =
    reply.type === 'result' && reply.result && 'checkpoint' in reply.result
      ? reply.result.checkpoint
      : undefined;
  worker.postMessage(reply, checkpoint ? [checkpoint] : []);
}
function finish(reply: EmulatorReply) {
  send({ type: 'trace', entries: camera.drain() });
  send(reply);
}
function progress(message: string) {
  send({
    type: 'trace',
    entries: [{ layer: Layer.Connection, level: Level.Info, direction: Direction.Host, message }],
  });
}
function readBytes(expression: string) {
  const proxy = py.runPython<PyProxy>(`memoryview(${expression})`);
  try {
    const view = proxy.getBuffer('u8');
    try {
      return view.data.slice();
    } finally {
      view.release();
    }
  } finally {
    proxy.destroy();
  }
}
async function transfer(
  kind: 'control-in' | 'control-out' | 'bulk-in' | 'bulk-out',
  options: object,
  data: Uint8Array = new Uint8Array(),
) {
  py.globals.set('usb_kind', kind);
  py.globals.set('usb_options', JSON.stringify(options));
  const buffer = py.toPy(data);
  py.globals.set('usb_data', buffer);
  try {
    py.runPython('usb_runtime.begin_transfer(usb_kind,json.loads(usb_options),bytes(usb_data))');
  } finally {
    py.globals.delete('usb_data');
    buffer.destroy();
  }
  const deadline = performance.now() + transferTimeoutMs;
  try {
    for (;;) {
      const step = JSON.parse(
        py.runPython<string>('usb_runtime.advance_transfer()'),
      ) as TransferStep;
      switch (step.state) {
        case 'read':
          return readBytes('usb_runtime.transfer_result');
        case 'written':
          return step.bytes;
        case 'pending':
          if (performance.now() >= deadline)
            throw new DOMException('Emulated USB transfer timed out', 'TimeoutError');
          // Keep timers and cancellation live while stock firmware waits for I/O.
          await new Promise((resolve) => setTimeout(resolve, 0));
          break;
        default:
          throw Error('Unknown emulated USB completion state');
      }
    }
  } catch (error) {
    failures = [
      ...failures.slice(-31),
      {
        name: error instanceof Error ? error.name : 'Error',
        message: String(error),
        stack: error instanceof Error ? error.stack : undefined,
      },
    ];
    // nusb maps WebUSB rejections to USB status; retain the simulator cause.
    send({
      type: 'trace',
      entries: [
        {
          layer: Layer.Usb,
          level: Level.Error,
          direction: Direction.Host,
          message: `Simulator USB: ${error instanceof Error ? error.message : String(error)}`,
        },
      ],
    });
    throw error;
  } finally {
    py.runPython('usb_runtime.end_transfer()');
  }
}

async function boot(request: Extract<EmulatorRequest, { call: EmulatorCall.Boot }>) {
  const firmware = request.firmware,
    config = firmware.emulate,
    usb = config?.usb;
  if (!config || !usb) throw Error('This firmware has no USB emulator configuration.');
  progress('Loading firmware emulator');
  const runtime = await loadCameraRuntime(new URL(usb.bundle, request.baseUrl).href);
  py = runtime.py;
  const digest = async (bytes: ArrayBuffer) =>
    [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
      .map((value) => value.toString(16).padStart(2, '0'))
      .join('');
  identity = {
    firmwareSha256: firmware.sha256,
    firmwareId: firmware.id,
    bundle: usb.bundle,
    nativeSha256: await digest(runtime.native),
    sourcesSha256: await digest(runtime.sources),
  };
  const inputs = await fetch(new URL('firmware-inputs.zip', new URL(usb.bundle, request.baseUrl)));
  if (!inputs.ok) throw Error('USB emulator inputs are unavailable.');
  py.unpackArchive(await inputs.arrayBuffer(), 'zip', { extractDir: '/repo' });
  const actual = [...new Uint8Array(await crypto.subtle.digest('SHA-256', request.dat))]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
  if (actual !== firmware.sha256 || request.dat.byteLength !== firmware.bytes)
    throw Error('Firmware differs from the selected catalog entry.');
  const path = config.boot.datPath;
  py.FS.mkdirTree(path.slice(0, path.lastIndexOf('/')));
  py.FS.writeFile(path, new Uint8Array(request.dat), { canOwn: true });
  py.globals.set(
    'usb_config',
    JSON.stringify({
      firmware: firmware.id,
      prepare: config.boot.prepare,
      module: usb.module,
      scenario: usb.scenario,
    }),
  );
  py.runPython(`
import hashlib,importlib,json
from pathlib import Path
for name,expected in json.loads(Path('/repo/web-inputs.json').read_text()).items():
    if hashlib.sha256((Path('/repo')/name).read_bytes()).hexdigest()!=expected:
        raise ValueError('USB emulator input SHA-256 differs: '+name)
config=json.loads(usb_config)
prepare=config['prepare']
getattr(importlib.import_module(prepare['module']),prepare['function'])(**prepare['options'])
usb_runtime=importlib.import_module(config['module'])
usb_runtime.start(config['firmware'],config['scenario'])
`);
  transferTimeoutMs = py.runPython<number>('usb_runtime.recipe["transfer_timeout_ms"]');
  let ready = false;
  for (let spent = 0; spent < config.boot.limit; spent += config.boot.step) {
    const state = JSON.parse(py.runPython<string>(`usb_runtime.advance(${config.boot.step})`)) as {
      ready: boolean;
      instructions: number;
    };
    progress(`Booting USB firmware · ${state.instructions.toLocaleString()} instructions`);
    if (state.ready) {
      ready = true;
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  if (!ready) throw Error('Firmware did not enable its USB controller within the boot budget.');
  progress('Enumerating emulated USB bus');
  const descriptor = JSON.parse(py.runPython<string>('usb_runtime.enumerate_device()')) as {
    device: number[];
    configurations: number[][];
    strings: Record<number, string>;
  };
  const device = (usbDevice = new EmulatedUsbDevice(
    {
      async controlIn(setup: UsbSetup, length: number) {
        return (await transfer('control-in', { setup, length })) as Uint8Array;
      },
      async controlOut(setup: UsbSetup, data: Uint8Array) {
        return (await transfer('control-out', { setup }, data)) as number;
      },
      async bulkIn(endpoint: number, length: number) {
        return (await transfer('bulk-in', { endpoint, length })) as Uint8Array;
      },
      async bulkOut(endpoint: number, data: Uint8Array) {
        return (await transfer('bulk-out', { endpoint }, data)) as number;
      },
    },
    Uint8Array.from(descriptor.device),
    descriptor.configurations.map((v) => Uint8Array.from(v)),
    descriptor.strings,
  ));
  Object.defineProperty(navigator, 'usb', { configurable: true, value: emulatedUsb(device) });
  await camera.initialize();
  const choice = await camera.pick();
  if (!choice) throw Error('Firmware did not enumerate as an FF80 device.');
  return {
    ...choice,
    firmware: {
      id: firmware.id,
      model: firmware.model,
      version: firmware.version,
      sha256: firmware.sha256,
    },
    name: `Simulator · ${firmware.model} ${firmware.version}`,
  };
}
async function handle(request: EmulatorRequest) {
  if (request.call !== EmulatorCall.Diagnostics) {
    trigger = {
      call: request.call,
      data: request.call === EmulatorCall.Submit ? Array.from(request.data) : undefined,
      guest_ticks:
        py &&
        py.runPython<unknown>(
          "usb_runtime.board.virtual_ticks if 'usb_runtime' in globals() and hasattr(usb_runtime, 'board') else None",
        ),
    };
    recentInputs = [...recentInputs.slice(-127), trigger];
  }
  try {
    let result;
    switch (request.call) {
      case EmulatorCall.Boot:
        result = await boot(request);
        break;
      case EmulatorCall.Open:
        link = await camera.open(request.interface);
        break;
      case EmulatorCall.Submit:
        if (!link) throw Error('USB session is not open.');
        await link.submit(request.data);
        break;
      case EmulatorCall.Poll:
        if (!link) throw Error('USB session is not open.');
        result = await link.poll();
        break;
      case EmulatorCall.ExportUsb:
        if (!link?.usable()) throw Error('No connected simulator');
        await link.close();
        link = undefined;
        exported = await exportUsbDevice(usbDevice);
        result = exported.descriptor;
        break;
      case EmulatorCall.Usb:
        if (!exported) throw Error('Simulator is not shared');
        result = await exported.execute(request.operation, request.timeout);
        break;
      case EmulatorCall.CloseUsb:
        await exported?.close();
        break;
      case EmulatorCall.Diagnostics: {
        if (!py) throw Error('Firmware emulator has not initialized.');
        result = JSON.parse(py.runPython<string>('usb_runtime.capture_report()'));
        const checkpoint = py.runPython<boolean>('usb_runtime.report_checkpoint is not None')
          ? readBytes('usb_runtime.report_checkpoint').buffer
          : undefined;
        py.runPython('usb_runtime.report_checkpoint = None');
        result = {
          ...result,
          checkpoint,
          identity,
          inputs: recentInputs,
          trigger,
          rawErrors: [...result.rawErrors, ...failures],
          runtime: { wasmMemoryBytes: py._module.HEAPU8.byteLength },
        };
        break;
      }
      case EmulatorCall.Close:
        if (!link?.usable()) throw Error('USB session is not ready');
        await link.close();
        link = undefined;
        break;
      case EmulatorCall.Abandon:
        link?.abandon();
        link = undefined;
        break;
    }
    finish({ type: 'result', id: request.id, result, usable: link?.usable() ?? false });
  } catch (error) {
    finish({
      type: 'error',
      id: request.id,
      message: error instanceof Error ? error.message : String(error),
      usable: link?.usable() ?? false,
    });
  }
}
let queue = Promise.resolve();
worker.onmessage = ({ data }: MessageEvent<EmulatorRequest>) => {
  queue = queue.then(() => handle(data));
};
