import { readFile } from 'node:fs/promises';
import { basename, dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { EmulatorSession, type SessionCommands } from './abi.ts';
import { copyBuffer } from './buffers.ts';
import {
  instantiateEmulatorModule,
  type EmulatorModuleFactory,
  type JitStatistics,
  type ModuleOptions,
} from './module.ts';

/** Load a generated module from disk and its adjacent Wasm binary. */
export async function loadNodeModule(modulePath: string, options: ModuleOptions = {}) {
  const absolutePath = resolve(modulePath);
  if (!absolutePath.endsWith('.mjs')) throw new Error('Emulator module path must end in .mjs.');
  const loaded: unknown = await import(pathToFileURL(absolutePath).href);
  if (loaded === null || typeof loaded !== 'object' || !('default' in loaded))
    throw new Error('Generated module has no factory.');
  const factory = loaded.default;
  if (typeof factory !== 'function') throw new Error('Generated module has no factory.');
  const wasmPath = resolve(dirname(absolutePath), basename(absolutePath, '.mjs') + '.wasm');
  const wasmBinary = await readFile(wasmPath);
  const wasmBytes = wasmBinary.buffer.slice(
    wasmBinary.byteOffset,
    wasmBinary.byteOffset + wasmBinary.byteLength,
  );
  return instantiateEmulatorModule(
    factory as EmulatorModuleFactory,
    wasmBytes,
    (path) => pathToFileURL(resolve(dirname(absolutePath), path)).href,
    options,
  );
}

export interface CameraStatus {
  ticks: string;
  instructions: string;
  failed: boolean;
}

export interface CameraFrame {
  width: number;
  height: number;
  output: number;
  sequence: string;
  rgba: Uint8Array<ArrayBuffer>;
}

export type Camera = Awaited<ReturnType<typeof openCamera>>;

/** Open a serialized camera session from its generated module and input files. */
export async function openCamera(
  modulePath: string,
  configPath: string,
  datPath: string,
  moduleOptions: ModuleOptions = {},
) {
  const module = await loadNodeModule(modulePath, moduleOptions);
  const configBytes = await readFile(configPath);
  const datBytes = await readFile(datPath);
  const config: unknown = JSON.parse(configBytes.toString('utf8'));
  return attachCamera(module, (session) => session.create(config, datBytes, 'session'));
}

/** Open a native prepared run: its document, update container and boot ROM files. */
export async function openPreparedCamera(
  modulePath: string,
  documentPath: string,
  updatePath: string,
  bootRomPath: string,
  moduleOptions: ModuleOptions = {},
) {
  const module = await loadNodeModule(modulePath, moduleOptions);
  const document = await readFile(documentPath, 'utf8');
  const update = await readFile(updatePath);
  const bootRom = await readFile(bootRomPath);
  return attachCamera(module, (session) =>
    session.create(document, update, 'prepared', { bootRom }),
  );
}

/** Open an exported running session without separate firmware or graph files. */
export async function openSnapshot(modulePath: string, snapshotPath: string) {
  const module = await loadNodeModule(modulePath);
  const bytes = await readFile(snapshotPath);
  return attachCamera(module, (session) => session.loadSnapshot(bytes));
}

function attachCamera(
  module: Awaited<ReturnType<typeof loadNodeModule>>,
  create: (session: EmulatorSession) => void,
) {
  let alive = true;
  const session = new EmulatorSession(module, () => {
    alive = false;
  });
  try {
    create(session);
  } catch (error) {
    if (alive) session.dispose();
    throw error;
  }

  const invoke = <T,>(operation: () => T): T => {
    if (!alive) throw new Error('Camera session is unavailable.');
    return operation();
  };

  return {
    command<K extends keyof SessionCommands>(
      action: K,
      options: SessionCommands[K]['request'] = {} as SessionCommands[K]['request'],
      binary: Uint8Array = new Uint8Array(),
    ) {
      const response = invoke(() => session.command(action, options, binary));
      return { json: response.result, bytes: response.bytes };
    },
    run(until: bigint) {
      invoke(() => session.run(until));
    },
    status(): CameraStatus {
      const status = invoke(() => session.status());
      return {
        ticks: status.ticks.toString(),
        instructions: status.instructions.toString(),
        failed: status.failed,
      };
    },
    scene(width: number, height: number, rgb: Uint8Array) {
      invoke(() => session.scene({ width, height, rgb }));
    },
    frame(output = 1): CameraFrame | undefined {
      const frame = invoke(() => session.frame(output));
      if (!frame) return undefined;
      return {
        width: frame.width,
        height: frame.height,
        output,
        sequence: frame.sequence.toString(),
        rgba: copyBuffer(frame.rgba),
      };
    },
    read(address: number | bigint, size: number): Uint8Array<ArrayBuffer> {
      if (typeof address === 'number' && !Number.isSafeInteger(address))
        throw new RangeError('Invalid memory span.');
      const parsedAddress = typeof address === 'bigint' ? address : BigInt(address);
      if (
        parsedAddress < 0n ||
        parsedAddress > 0xffffffffn ||
        !Number.isSafeInteger(size) ||
        size < 0 ||
        parsedAddress + BigInt(size) > 0x1_0000_0000n
      )
        throw new RangeError('Invalid memory span.');
      return invoke(
        () => session.command('memory_read', { address: parsedAddress.toString(), size }).bytes,
      );
    },
    capacity() {
      return invoke(() => module.HEAPU8.buffer.byteLength);
    },
    jitStats(): JitStatistics {
      return invoke(() => module.tcgJitStats());
    },
    alive() {
      return alive;
    },
    close() {
      if (!alive) return;
      try {
        session.dispose();
      } finally {
        alive = false;
      }
    },
  };
}

export type { JitOptions, JitStatistics, ModuleOptions } from './module.ts';
export type { SessionCommands } from './abi.ts';
