import type {
  EmulatorManifest,
  EmulatorModule,
  SessionCommands,
} from '../../dist/emulator/index.js';
import type {
  Camera,
  CameraFrame,
  CameraStatus,
  JitOptions,
  JitStatistics,
  ModuleOptions,
  SessionCommands as NodeSessionCommands,
} from '../../dist/emulator/node.mjs';

declare const camera: Camera;
declare const module: EmulatorModule;

const manifest: EmulatorManifest = {
  schema: 1,
  abi: 1,
  module: 'camera.mjs',
  wasm: 'camera.wasm',
  construction: 'session',
  session: {},
  ticksPerSecond: 12_000_000,
  bootTicks: 0,
  output: 1,
};
const status: { ticks: string; instructions: string; failed: boolean } = camera.status();
const frame: CameraFrame | undefined = camera.frame();
const cameraStatus: CameraStatus = camera.status();
const jitOptions: JitOptions = { threshold: 2 };
const moduleOptions: ModuleOptions = { tcgJit: jitOptions };
const jitStatistics: JitStatistics = camera.jitStats();
const memory: { address: string; size: number } = camera.command('memory_read', {
  address: '4096',
  size: 4,
}).json;
const stats = camera.jitStats();
const count: number = stats.enabled ? stats.installed : 0;
type MemoryRead = SessionCommands['memory_read']['request'];
type NodeMemoryRead = NodeSessionCommands['memory_read']['request'];
const request: MemoryRead = { address: '8192', size: 8 };
const nodeRequest: NodeMemoryRead = request;
const heap: Uint8Array = module.HEAPU8;
// @ts-expect-error Unknown ABI commands are rejected.
camera.command('start');
// @ts-expect-error Binary command input stays Uint8Array.
camera.command('replay', {}, new ArrayBuffer(1));
// @ts-expect-error Manifest ABI versions are exact.
const invalid: EmulatorManifest = { ...manifest, abi: 2 };
void status;
void frame;
void cameraStatus;
void moduleOptions;
void jitStatistics;
void memory;
void count;
void request;
void nodeRequest;
void heap;
void invalid;
