export {
  EmulatorSession,
  GuestExecutionError,
  UnsupportedEmulatorOperation,
  type EmulatorModule,
  type SaveResult,
  type SessionCommands,
  type SessionState,
  type UsbStatus,
} from './abi.ts';
export {
  loadEmulatorModule,
  parseEmulatorManifest,
  instantiateEmulatorModule,
  type EmulatorManifest,
  type EmulatorModuleFactory,
  type EnabledJitStatistics,
  type JitOptions,
  type JitStatistics,
  type LoadedEmulatorModule,
  type ModuleFactoryOptions,
  type ModuleOptions,
} from './module.ts';
import { loadEmulatorModule, type ModuleOptions } from './module.ts';
export {
  type BootInput,
  type CardEntry,
  type CardReadOptions,
  type CardState,
  type CardTree,
  type Checkpoint,
  type Connection,
  type EngineFingerprint,
  type Input,
  type TouchPhase,
} from './contracts.ts';
export { HostTimings } from './host-timings.ts';
export { createBufferSession } from './buffers.ts';

/** Load the generated runtime artifacts staged beside this package entry. */
export function loadBundledEmulatorModule(
  options: ModuleOptions = {},
  fetcher: typeof fetch = fetch,
) {
  return loadEmulatorModule(
    new URL('./camera.mjs', import.meta.url),
    new URL('./camera.wasm', import.meta.url),
    options,
    fetcher,
  );
}
