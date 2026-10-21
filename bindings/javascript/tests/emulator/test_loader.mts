// Generated ARM exercises LZ and memory DMA register interfaces through the C ABI.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { loadNodeModule } from '../../dist/emulator/node.mjs';
import type { EmulatorModule } from '../../dist/emulator/index.js';
import { finishJit, moduleOptions } from './test_mode.mts';

interface GuestModule extends EmulatorModule {
  _camera_session_create: NonNullable<EmulatorModule['_camera_session_create']>;
  _camera_command: NonNullable<EmulatorModule['_camera_command']>;
}

interface Literal {
  at: number;
  register: number;
  value: number;
}

interface MemoryState {
  registers: number[];
  pending: unknown;
}

interface LzState {
  pending: unknown;
  regs: number[];
}

interface DeviceReport {
  kind: string;
  state: {
    modes?: { name: string; state: { state: MemoryState } }[];
    state?: LzState;
  };
}

interface SessionReport {
  bus: { devices: DeviceReport[] };
  instructions: number;
  timeline: unknown;
}

interface CommandReport {
  report: SessionReport;
}

interface Result {
  memory: MemoryState;
  lz: LzState;
  instructions: number;
  ticks: unknown;
}

const modulePath =
  process.argv[2] ??
  fileURLToPath(new URL('../../../../target/emulator/camera.mjs', import.meta.url));
const loaded = await loadNodeModule(modulePath, moduleOptions({ threshold: 1, batchSize: 1 }));
if (!loaded._camera_session_create || !loaded._camera_command)
  throw new Error('The generated module does not expose session commands.');
const module = loaded as GuestModule;

const words: number[] = [];
const literals: Literal[] = [];
const branches: number[] = [];

function literal(register: number, value: number): void {
  literals.push({ at: words.length, register, value });
  words.push(0);
}

function store(address: number, value: number): void {
  literal(0, address);
  literal(1, value);
  words.push(0xe5801000, ...Array<number>(8).fill(0xe1a00000));
}

function compare(address: number, expected: number, byte = false): void {
  literal(0, address);
  words.push(byte ? 0xe5d02000 : 0xe5902000, 0xe3520000 | expected);
  branches.push(words.length);
  words.push(0);
}

for (const [offset, value] of [
  [0, 0],
  [0x80, 0x30003000],
  [0x8c, 1],
  [0x40, 0x12000],
  [0x48, 0x14000],
  [0x44, 6],
  [4, 2],
])
  store(0xfff43000 + offset, value);
compare(0xfff4304c, 18);
compare(0xfff43088, 1);
for (let index = 0; index < 18; index++) compare(0x14000 + index, [97, 98, 99][index % 3]!, true);
compare(0x14012, 0xcc, true);
store(0xfff43088, 1);

for (const [offset, value] of [
  [0, 0x15000],
  [4, 0x16000],
  [8, 64],
  [0x2c, 0x1555400],
  [0x28, 5],
])
  store(0xfff02000 + offset, value);
compare(0xfff02024, 0x40);
for (let index = 0; index < 64; index++)
  compare(0x16000 + index, [0x78, 0x56, 0x34, 0x12][index % 4]!, true);
compare(0x16040, 0xcc, true);
store(0xfff0200c, 0x600d);
words.push(0xeafffffe);

const failure = words.length;
store(0xfff0200c, 0xbad);
words.push(0xeafffffe);
for (const at of branches) words[at] = 0x1a000000 | ((failure - at - 2) & 0xffffff);
const pool = words.length;
for (let index = 0; index < literals.length; index++) {
  const { at, register, value } = literals[index]!;
  const offset = (pool + index - at - 2) * 4;
  assert(offset >= 0 && offset < 4096);
  words[at] = 0xe59f0000 | (register << 12) | offset;
  words.push(value);
}

const firmware = new Uint8Array(0x8000).fill(0xcc);
const firmwareView = new DataView(firmware.buffer);
words.forEach((value, index) => firmwareView.setUint32(index * 4, value, true));
firmware.set([3, 97, 98, 99, 0x80, 0x3f], 0x2000);
for (let index = 0; index < 32; index++)
  firmware[0x5000 + index] = [0x78, 0x56, 0x34, 0x12][index % 4]!;

const hash = (bytes: Uint8Array): string => createHash('sha256').update(bytes).digest('hex');
const node = (name: string, model: object, windows: { base: number; size: number }[]) => ({
  name,
  boot_entries: ['loader'],
  windows,
  dependencies: [],
  model,
});

function configuration(quantum: number) {
  const mapping = { address: 0x10000, physical: 0x10000, size: 0x10000, permissions: 7 };
  return {
    schema: 1,
    firmwareId: 'generated',
    bootSource: 'internal',
    initialPositions: {},
    clockSeconds2000: 0,
    graph: {
      schema: 1,
      firmwareId: 'generated',
      bootSource: 'internal',
      clockSeconds2000: 0,
      quantum,
      translationCacheBytes: 1 << 20,
      execution: {
        processors: [
          {
            model: 'cortex-r5f',
            released: true,
            registers: [['Pc', 0x10000]],
            mprofile: null,
            rprofile: {
              cp14_identity: 0x4003,
              tcm: { atcm: { bytes: 0, reset: 0 }, btcm: { bytes: 0, reset: 0 } },
            },
          },
        ],
        ram: [{ base: 0x10000, size: 0x10000 }],
        mappings: [{ core: 0, ...mapping }],
        dma_mappings: [{ ...mapping, permissions: 3 }],
        decoders: [],
        instructions_per_tick: [1, 1],
        peripherals: [0xfffb3000, 0xfff02000, 0xfff43000].map((address) => ({
          core: 0,
          address,
          physical: address,
          size: 4096,
        })),
      },
      devices: {
        schema: 1,
        boot_entry: 'loader',
        wires: { routing: 'irq-routes' },
        nodes: [
          node(
            'irq',
            {
              kind: 'exr2-interrupt-controller',
              options: { arbitration: 'unique' },
              core: 0,
              base: 0xfffb3000,
              routes: 'routing',
              software: [],
            },
            [{ base: 0xfffb3000, size: 4096 }],
          ),
          node('lz', { kind: 'lz', options: {}, base: 0xfff43000, controllers: ['irq'] }, [
            { base: 0xfff43000, size: 256 },
          ]),
          node('memory', { kind: 'memory-dma', options: {}, base: 0xfff02000, routes: 'routing' }, [
            { base: 0xfff02000, size: 64 },
            { base: 0xfff02300, size: 4 },
          ]),
        ],
      },
      firmware: {
        size: firmware.length,
        sha256: hash(firmware),
        format: 'raw',
        mappings: [
          {
            name: 'program',
            offset: 0,
            size: firmware.length,
            base: 0x10000,
            encoding: 'raw',
            executable: true,
          },
        ],
      },
    },
  };
}

const view = (): DataView => new DataView(module.HEAPU8.buffer);
const output = module._camera_alloc(32);
if (!output) throw new Error('Could not allocate a result descriptor.');

function upload(bytes: Uint8Array): number {
  const pointer = module._camera_alloc(bytes.length);
  if (!pointer) throw new Error('Could not allocate an input buffer.');
  module.HEAPU8.set(bytes, pointer);
  return pointer;
}

function span(at: number): Uint8Array {
  const descriptor = view();
  const pointer = descriptor.getUint32(at, true);
  const length = descriptor.getUint32(at + 4, true);
  if (pointer + length > module.HEAPU8.byteLength) throw new Error('Invalid result span.');
  return module.HEAPU8.subarray(pointer, pointer + length);
}

function error(handle = 0): string {
  module._camera_error(handle, output);
  return new TextDecoder().decode(span(output));
}

let previous: Result | undefined;
for (const quantum of [17, 1000]) {
  const json = new TextEncoder().encode(JSON.stringify(configuration(quantum)));
  const configPointer = upload(json);
  const firmwarePointer = upload(firmware);
  const handle = module._camera_session_create(
    configPointer,
    json.length,
    firmwarePointer,
    firmware.length,
  );
  assert(handle, error());
  module._camera_free(configPointer, json.length);
  module._camera_free(firmwarePointer, firmware.length);
  try {
    assert.equal(module._camera_run_until(handle, 2000n), 0, error(handle));
    const request = new TextEncoder().encode(
      JSON.stringify({ action: 'error_report', options: {} }),
    );
    const requestPointer = upload(request);
    try {
      const code = module._camera_command(handle, requestPointer, request.length, 0, 0, output);
      assert.equal(code, 0, code === 0 ? '' : error(handle));
      const report = (JSON.parse(new TextDecoder().decode(span(output))) as CommandReport).report;
      const controller = report.bus.devices.find((device) => device.kind === 'exr2-dma-controller');
      assert(controller?.state.modes);
      const memoryMode = controller.state.modes.find((mode) => mode.name === 'exr2-memory-dma');
      assert(memoryMode);
      const memory = memoryMode.state.state;
      const lzDevice = report.bus.devices.find((device) => device.kind === 'exr2-lz');
      assert(lzDevice?.state.state);
      const lz = lzDevice.state.state;
      assert.equal(memory.registers[3], 0x600d, 'guest output/canary assertion');
      assert.equal(memory.pending, null);
      assert.equal(lz.pending, null);
      assert.equal(lz.regs[0x4c / 4], 18);
      assert.equal(lz.regs[0x88 / 4], 0);

      const result: Result = {
        memory,
        lz,
        instructions: report.instructions,
        ticks: report.timeline,
      };
      if (previous) assert.deepEqual(result, previous);
      previous = result;
    } finally {
      module._camera_free(requestPointer, request.length);
    }
  } finally {
    assert.equal(module._camera_destroy(handle), 0);
  }
}

module._camera_free(output, 32);
console.log(JSON.stringify({ passed: true, quantums: [17, 1000], lzBytes: 18, fillBytes: 64 }));
finishJit(module);
