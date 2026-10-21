import type { EmulatorModule } from './abi.ts';
import type { BootInput, Input } from './contracts.ts';

export interface JitOptions {
  threshold?: number;
  batchSize?: number;
  maxFunctions?: number;
  maxBytes?: number;
}

export interface ModuleOptions {
  /** Shared board/CPU policy. Interactive is the feature-exploration default. */
  executionPolicy?: 'interactive' | 'exact';
  tcgJit?: boolean | JitOptions;
}

export interface EnabledJitStatistics {
  enabled: true;
  options: { maxFunctions: number };
  installed: number;
  compiled: number;
  flushes: number;
  capacityRefusals: number;
  unsupported: number;
  cacheHits: number;
  compileMilliseconds: number;
  prepareMilliseconds: number;
  chainedExecutions: string;
  compiledExecutions: string;
  entries: string;
  failures: number;
  lastError: string | null;
}

export type JitStatistics = { enabled: false } | EnabledJitStatistics;

export interface ModuleFactoryOptions extends ModuleOptions {
  wasmBinary: ArrayBuffer;
  locateFile(path: string): string;
}

export type EmulatorModuleFactory = (options: ModuleFactoryOptions) => Promise<unknown>;

const requiredFunctions = [
  '_camera_alloc',
  '_camera_free',
  '_camera_create',
  '_camera_session_create',
  '_camera_command',
  '_camera_scene_buffer',
  '_camera_scene_commit',
  '_camera_destroy',
  '_camera_run_until',
  '_camera_input',
  '_camera_status',
  '_camera_frame',
  '_camera_error',
  'tcgJitStats',
] as const;

export type LoadedEmulatorModule = EmulatorModule &
  Required<
    Pick<
      EmulatorModule,
      '_camera_session_create' | '_camera_command' | '_camera_scene_buffer' | '_camera_scene_commit'
    >
  >;

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object';
}

function moduleFrom(value: unknown): LoadedEmulatorModule {
  if (!record(value) || !(value.HEAPU8 instanceof Uint8Array))
    throw new Error('Generated module has no Wasm heap.');
  for (const name of requiredFunctions) {
    if (typeof value[name] !== 'function') throw new Error(`Missing C ABI export: ${name}.`);
  }
  return value as unknown as LoadedEmulatorModule;
}

/** Instantiate a generated module while keeping the Wasm binary independently addressable. */
export async function instantiateEmulatorModule(
  factory: EmulatorModuleFactory,
  wasmBinary: ArrayBuffer,
  locateFile: (path: string) => string,
  options: ModuleOptions = {},
): Promise<LoadedEmulatorModule> {
  return moduleFrom(await factory({ ...options, wasmBinary, locateFile }));
}

/** Load an Emscripten module and its Wasm binary in browsers or workers. */
export async function loadEmulatorModule(
  moduleUrl: string | URL,
  wasmUrl: string | URL,
  options: ModuleOptions = {},
  fetcher: typeof fetch = fetch,
): Promise<LoadedEmulatorModule> {
  const base = globalThis.location?.href;
  const moduleHref =
    moduleUrl instanceof URL
      ? moduleUrl.href
      : base
        ? new URL(moduleUrl, base).href
        : new URL(moduleUrl).href;
  const wasmHref = new URL(wasmUrl, moduleHref).href;
  const [moduleResponse, wasmResponse] = await Promise.all([
    import(/* @vite-ignore */ moduleHref),
    fetcher(wasmHref),
  ]);
  if (!wasmResponse.ok) throw new Error(`Wasm binary request failed (${wasmResponse.status}).`);
  const loaded: unknown = moduleResponse;
  if (!record(loaded) || typeof loaded.default !== 'function')
    throw new Error('Generated module has no factory.');
  return instantiateEmulatorModule(
    loaded.default as EmulatorModuleFactory,
    await wasmResponse.arrayBuffer(),
    (path) => new URL(path, wasmHref).href,
    options,
  );
}

/** Validate the versioned manifest before fetching a module or binary. */
export function parseEmulatorManifest(value: unknown): EmulatorManifest {
  if (!record(value)) throw new Error('Invalid emulator manifest.');
  const manifest = value as unknown as EmulatorManifest;
  if (
    manifest.schema !== 1 ||
    manifest.abi !== 1 ||
    typeof manifest.module !== 'string' ||
    manifest.module.length === 0 ||
    typeof manifest.wasm !== 'string' ||
    manifest.wasm.length === 0 ||
    !['resolved', 'session'].includes(manifest.construction) ||
    (manifest.executionPolicy !== undefined &&
      !['interactive', 'exact'].includes(manifest.executionPolicy)) ||
    !record(manifest.session) ||
    !Number.isSafeInteger(manifest.ticksPerSecond) ||
    manifest.ticksPerSecond <= 0 ||
    !Number.isSafeInteger(manifest.bootTicks) ||
    manifest.bootTicks < 0 ||
    !Number.isSafeInteger(manifest.output) ||
    manifest.output < 0 ||
    manifest.output > 0xffffffff
  )
    throw new Error('Unsupported emulator manifest.');
  if (manifest.observations !== undefined) {
    if (
      !Array.isArray(manifest.observations) ||
      manifest.observations.length > 64 ||
      manifest.observations.some(
        (observation: unknown) =>
          !record(observation) ||
          typeof observation.name !== 'string' ||
          !/^[a-zA-Z][a-zA-Z0-9_]*$/.test(observation.name) ||
          typeof observation.address !== 'string' ||
          !/^[0-9]+$/.test(observation.address) ||
          !Number.isSafeInteger(observation.size) ||
          (observation.size as number) < 1 ||
          (observation.size as number) > 4096,
      )
    )
      throw new Error('Invalid observation declaration.');
  }
  if (manifest.inputs !== undefined && !Array.isArray(manifest.inputs))
    throw new Error('Invalid emulator inputs.');
  if (
    manifest.usb !== undefined &&
    (!record(manifest.usb) || manifest.usb.kind !== 'exr2' || typeof manifest.usb.node !== 'string')
  )
    throw new Error('Invalid emulator USB configuration.');
  return manifest;
}

export interface EmulatorManifest {
  schema: 1;
  abi: 1;
  module: string;
  wasm: string;
  construction: 'resolved' | 'session';
  /** Override the default policy for native-parity qualification bundles. */
  executionPolicy?: ModuleOptions['executionPolicy'];
  session: Record<string, unknown>;
  ticksPerSecond: number;
  bootTicks: number;
  output: number;
  inputs?: Input[];
  /** Optional external endpoint and boot inputs configured by the caller. */
  usb?: {
    kind: 'exr2';
    node: string;
    bootInputs?: BootInput[];
    initialPositions?: Record<string, string>;
  };
  observations?: { name: string; address: string; size: number }[];
}
