import { loadCameraRuntime, type Pyodide, type PyProxy } from '../engine/runtime';
import { EmulatedUsbDevice, emulatedUsb, type UsbSetup } from '../engine/webusb-device';
import { WebUsbCamera, type CameraLink } from './camera';
import { EmulatorCall, type EmulatorRequest, type EmulatorReply } from './emulator-protocol';
import { Direction, Layer, Level } from './log';
const worker = self as unknown as DedicatedWorkerGlobalScope;
const camera = new WebUsbCamera();
let py: Pyodide;
let link: CameraLink | undefined;
function send(reply: EmulatorReply) {
  worker.postMessage(reply);
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
function transfer(
  kind: 'control-in' | 'control-out' | 'bulk-in' | 'bulk-out',
  options: object,
  data: Uint8Array = new Uint8Array(),
) {
  py.globals.set('usb_kind', kind);
  py.globals.set('usb_options', JSON.stringify(options));
  const buffer = py.toPy(data);
  py.globals.set('usb_data', buffer);
  try {
    if (kind === 'control-in' || kind === 'bulk-in')
      return readBytes('usb_runtime.transfer(usb_kind,json.loads(usb_options))');
    return py.runPython<number>(
      'usb_runtime.transfer(usb_kind,json.loads(usb_options),bytes(usb_data))',
    );
  } finally {
    py.globals.delete('usb_data');
    buffer.destroy();
  }
}
async function boot(request: Extract<EmulatorRequest, { call: EmulatorCall.Boot }>) {
  const firmware = request.firmware,
    config = firmware.emulate,
    usb = config?.usb;
  if (!config || !usb) throw Error('This firmware has no USB emulator configuration.');
  progress('Loading firmware emulator');
  ({ py } = await loadCameraRuntime(new URL(usb.bundle, request.baseUrl).href));
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
  const device = new EmulatedUsbDevice(
    {
      async controlIn(setup: UsbSetup, length: number) {
        return transfer('control-in', { setup, length }) as Uint8Array;
      },
      async controlOut(setup: UsbSetup, data: Uint8Array) {
        return transfer('control-out', { setup }, data) as number;
      },
      async bulkIn(endpoint: number, length: number) {
        return transfer('bulk-in', { endpoint, length }) as Uint8Array;
      },
      async bulkOut(endpoint: number, data: Uint8Array) {
        return transfer('bulk-out', { endpoint }, data) as number;
      },
    },
    Uint8Array.from(descriptor.device),
    descriptor.configurations.map((v) => Uint8Array.from(v)),
    descriptor.strings,
  );
  Object.defineProperty(navigator, 'usb', { configurable: true, value: emulatedUsb(device) });
  await camera.initialize();
  const choice = await camera.pick();
  if (!choice) throw Error('Firmware did not enumerate as an FF80 device.');
  return { ...choice, name: `Simulator · ${firmware.model} ${firmware.version}` };
}
async function handle(request: EmulatorRequest) {
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
