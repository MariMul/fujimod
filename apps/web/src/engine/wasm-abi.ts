import type { BootInput, CardReadOptions, CardState, CardTree, Connection, Input } from './api';

/** Export names and wasm32 layouts are versioned independently of session JSON. */
export interface CameraModule {
  HEAPU8: Uint8Array;
  _camera_alloc(this: void, length: number): number;
  _camera_free(this: void, pointer: number, length: number): void;
  _camera_create(
    this: void,
    config: number,
    configLength: number,
    dat: number,
    datLength: number,
  ): number;
  _camera_destroy(this: void, handle: number): number;
  _camera_run_until(this: void, handle: number, tick: bigint): number;
  _camera_input(
    this: void,
    handle: number,
    key: number,
    keyLength: number,
    pressed: number,
    at: bigint,
  ): number;
  _camera_status(this: void, handle: number, out: number): number;
  _camera_frame(this: void, handle: number, output: number, out: number): number;
  _camera_error(this: void, handle: number, out: number): number;
  /** Session extensions: absent exports refuse the operation, never emulate success. */
  _camera_session_create?: CameraModule['_camera_create'];
  _camera_command?: (
    handle: number,
    json: number,
    jsonLength: number,
    bytes: number,
    length: number,
    out: number,
  ) => number;
  _camera_scene_buffer?: (handle: number, width: number, height: number, out: number) => number;
  _camera_scene_commit?: (handle: number, enabled: number) => number;
}
export interface WasmManifest {
  schema: 1;
  abi: 1;
  module: string;
  wasm: string;
  construction: 'resolved' | 'session';
  session: Record<string, unknown>;
  ticksPerSecond: number;
  bootTicks: number;
  output: number;
}
export interface SessionState {
  inputs: Input[];
  positions: Record<string, string>;
  connections: Connection[];
  card: CardState;
  poweredOn: boolean;
}
export interface SaveResult {
  durable: { offset: number; length: number };
  snapshot?: { offset: number; length: number };
  positions: Record<string, string>;
  poweredOn: boolean;
}
/** Each command has one JSON request/result and an optional opaque binary span. */
export interface SessionCommands {
  state: { request: Record<string, never>; result: SessionState };
  press: {
    request: { id: string; stage?: string; point?: { x: number; y: number } };
    result: SessionState;
  };
  release: { request: { id: string }; result: SessionState };
  set_position: { request: { id: string; position: string | number }; result: SessionState };
  power: { request: { on: boolean }; result: SessionState };
  power_cycle: { request: { inputs: BootInput[] }; result: SessionState };
  card_state: { request: Record<string, never>; result: CardState };
  card_tree: { request: { revision?: number }; result: CardTree };
  card_read: { request: { path: string } & CardReadOptions; result: Record<string, never> };
  card_insert: { request: { present: boolean }; result: SessionState };
  card_format: { request: Record<string, never>; result: SessionState };
  restart_card: {
    request: { files: { path: string; offset: number; length: number }[] };
    result: SessionState;
  };
  save: { request: Record<string, never>; result: SaveResult };
  restore: {
    request: {
      durable: { offset: number; length: number };
      snapshot?: { offset: number; length: number };
      positions: Record<string, string>;
      poweredOn: boolean;
    };
    result: SessionState;
  };
  rollback: { request: { seconds: number }; result: SessionState };
  profile: { request: { limit?: number }; result: Record<string, unknown> };
  error_report: {
    request: Record<string, never>;
    result: { report: Record<string, unknown>; snapshot?: { offset: number; length: number } };
  };
}
export class UnsupportedWasmOperation extends Error {}
export class GuestExecutionError extends Error {}

/** Owns allocations; every returned pixel span borrows the current Wasm heap. */
export class WasmSession {
  private scratch: number;
  private handle = 0;
  constructor(readonly module: CameraModule) {
    this.scratch = module._camera_alloc(32);
    if (!this.scratch || this.scratch % 8) throw Error('Invalid Wasm descriptor allocation.');
  }
  get created() {
    return this.handle !== 0;
  }
  private view() {
    return new DataView(this.module.HEAPU8.buffer);
  }
  private span(descriptor: number): Uint8Array {
    const view = this.view();
    return this.bytes(view.getUint32(descriptor, true), view.getUint32(descriptor + 4, true));
  }
  private bytes(pointer: number, length: number) {
    const heap = this.module.HEAPU8;
    if (pointer + length > heap.byteLength || (length > 0 && pointer === 0))
      throw Error('Invalid Wasm span.');
    return heap.subarray(pointer, pointer + length);
  }
  private upload<T>(bytes: Uint8Array, run: (pointer: number, length: number) => T): T {
    if (!bytes.length) return run(0, 0);
    const pointer = this.module._camera_alloc(bytes.length);
    if (!pointer) throw Error('Wasm allocation failed.');
    try {
      this.module.HEAPU8.set(bytes, pointer);
      return run(pointer, bytes.length);
    } finally {
      this.module._camera_free(pointer, bytes.length);
    }
  }
  private json<T>(value: unknown, run: (pointer: number, length: number) => T) {
    return this.upload(new TextEncoder().encode(JSON.stringify(value)), run);
  }
  private error(handle = this.handle) {
    if (this.module._camera_error(handle, this.scratch) !== 0) return 'Wasm error unavailable.';
    return new TextDecoder().decode(this.span(this.scratch));
  }
  private check(code: number) {
    if (code !== 0) throw Error(this.error());
  }
  create(config: unknown, dat: Uint8Array, construction: WasmManifest['construction']) {
    if (this.handle) throw Error('Session already created.');
    const create =
      construction === 'resolved' ? this.module._camera_create : this.module._camera_session_create;
    if (!create) throw new UnsupportedWasmOperation('Missing C ABI export: camera_session_create.');
    this.handle = this.json(config, (p, n) => this.upload(dat, (d, l) => create(p, n, d, l)));
    if (!this.handle) throw Error(this.error(0));
  }
  status() {
    this.check(this.module._camera_status(this.handle, this.scratch));
    const v = this.view();
    return {
      ticks: v.getBigUint64(this.scratch, true),
      instructions: v.getBigUint64(this.scratch + 8, true),
      failed: v.getUint32(this.scratch + 16, true) !== 0,
    };
  }
  run(until: bigint) {
    const result = this.module._camera_run_until(this.handle, until);
    if (result !== 0) throw new GuestExecutionError(this.error());
  }
  input(key: string, pressed: boolean) {
    const { ticks } = this.status();
    this.jsonString(key, (p, n) =>
      this.check(this.module._camera_input(this.handle, p, n, Number(pressed), ticks)),
    );
  }
  private jsonString<T>(value: string, run: (p: number, n: number) => T) {
    return this.upload(new TextEncoder().encode(value), run);
  }
  frame(output: number) {
    const code = this.module._camera_frame(this.handle, output, this.scratch);
    if (code === 1) return undefined;
    this.check(code);
    const v = this.view(),
      p = this.scratch;
    const width = v.getUint32(p + 8, true),
      height = v.getUint32(p + 12, true);
    const sequence = v.getBigUint64(p + 24, true);
    const rgba = this.span(p);
    if (!width || !height || rgba.length !== width * height * 4)
      throw Error('Invalid Wasm frame dimensions.');
    return { rgba, width, height, sequence };
  }
  command<K extends keyof SessionCommands>(
    action: K,
    options: SessionCommands[K]['request'],
    bytes = new Uint8Array(),
  ): { result: SessionCommands[K]['result']; bytes: Uint8Array<ArrayBuffer> } {
    const command = this.module._camera_command;
    if (!command)
      throw new UnsupportedWasmOperation(`Missing C ABI export: camera_command (${action}).`);
    return this.json({ action, options }, (p, n) =>
      this.upload(bytes, (b, l) => {
        const code = command(this.handle, p, n, b, l, this.scratch);
        if (code !== 0) {
          const message = this.error();
          if (this.status().failed) throw new GuestExecutionError(message);
          throw Error(message);
        }
        // Decode before freeing uploads: free is allowed to invalidate borrowed results.
        const result = JSON.parse(
          new TextDecoder().decode(this.span(this.scratch)),
        ) as SessionCommands[K]['result'];
        return { result, bytes: this.span(this.scratch + 8).slice() };
      }),
    );
  }
  scene(scene: { width: number; height: number; rgb: Uint8Array } | null) {
    const { _camera_scene_buffer: reserve, _camera_scene_commit: commit } = this.module;
    if (!reserve || !commit)
      throw new UnsupportedWasmOperation(
        'Missing C ABI exports: camera_scene_buffer/camera_scene_commit.',
      );
    if (scene) {
      const { width, height, rgb } = scene;
      if (
        !Number.isSafeInteger(width) ||
        !Number.isSafeInteger(height) ||
        width <= 0 ||
        height <= 0 ||
        rgb.length !== width * height * 3
      )
        throw Error('Invalid scene dimensions.');
      this.check(reserve(this.handle, width, height, this.scratch));
      const target = this.span(this.scratch);
      if (target.length !== rgb.length) throw Error('Invalid Wasm scene span.');
      target.set(rgb);
    }
    this.check(commit(this.handle, Number(scene !== null)));
  }
  dispose() {
    try {
      if (this.handle) this.check(this.module._camera_destroy(this.handle));
    } finally {
      this.handle = 0;
      this.module._camera_free(this.scratch, 32);
      this.scratch = 0;
    }
  }
}
