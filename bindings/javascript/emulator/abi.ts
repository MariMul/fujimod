import { HostTimings } from './host-timings.ts';
import type {
  BootInput,
  CardReadOptions,
  CardState,
  CardTree,
  Connection,
  Input,
  Checkpoint,
  TouchPhase,
  EngineFingerprint,
} from './contracts.ts';
import type { EmulatorManifest } from './module.ts';

/** Export names and wasm32 layouts are versioned independently of session JSON. */
export interface EmulatorModule {
  HEAPU8: Uint8Array;
  tcgJitStats(): import('./module.js').JitStatistics;
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
  _camera_session_create?: EmulatorModule['_camera_create'];
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
export interface UsbStatus {
  connected: boolean;
  endpoint: {
    options: {
      packet_bytes: number;
      out_endpoints: [number, number][];
      in_endpoints: [number, number][];
    };
    registers: number[];
    rx_length: number[];
    rx_position: number[];
    setup_length: number;
    setup_position: number;
    tx_packets: number[];
    active_dma: boolean;
  };
}
export interface SessionState {
  inputs: Input[];
  positions: Record<string, string>;
  connections: Connection[];
  card: CardState;
  poweredOn: boolean;
  inputQueue?: { pending: number; active: unknown; completed: number };
}
export interface SaveResult {
  format: 'camera-media-cbor';
  schema: 1;
  durable: { offset: number; length: number };
  positions: Record<string, string>;
  poweredOn: boolean;
}
/** Each command has one JSON request/result and an optional opaque binary span. */
export interface SessionCommands {
  usb_status: { request: { node: string }; result: UsbStatus };
  usb_reset: { request: { node: string }; result: Record<string, never> };
  usb_setup: { request: { node: string }; result: Record<string, never> };
  usb_out: { request: { node: string; endpoint: number }; result: Record<string, never> };
  usb_in: {
    request: { node: string; endpoint: number; size: number };
    result: Record<string, never>;
  };
  usb_in_token: { request: { node: string; endpoint: number }; result: Record<string, never> };
  state: { request: Record<string, never>; result: SessionState };
  press: {
    request: { id: string; stage?: string };
    result: SessionState;
  };
  release: { request: { id: string }; result: SessionState };
  rotate: { request: { id: string; steps: number }; result: SessionState };
  touch: { request: { id: string; phase: TouchPhase; x: number; y: number }; result: SessionState };
  set_position: { request: { id: string; position: string }; result: SessionState };
  power: { request: { on: boolean }; result: SessionState };
  power_cycle: { request: { inputs: BootInput[] }; result: SessionState };
  card_state: { request: Record<string, never>; result: CardState };
  card_tree: { request: { revision?: number }; result: CardTree };
  card_read: { request: { path: string } & CardReadOptions; result: Record<string, never> };
  card_insert: { request: { present: boolean }; result: SessionState };
  card_format: { request: Record<string, never>; result: SessionState };
  restart_card: {
    request: { files: { path: string; offset: number; length: number }[]; inputs?: BootInput[] };
    result: SessionState;
  };
  save: { request: Record<string, never>; result: SaveResult };
  restore_durable: { request: Record<string, never>; result: SessionState };
  checkpoint: { request: Record<string, never>; result: Checkpoint & { retainedIds: string[] } };
  restore: { request: { id: string }; result: { tick: string; instructions: string[] } };
  undo: { request: Record<string, never>; result: { tick: string; instructions: string[] } };
  rollback: { request: { tick: string }; result: { tick: string; instructions: string[] } };
  history: {
    request: Record<string, never>;
    result: { format: 'session-input-log-cbor'; schema: 1; entries: number };
  };
  replay: { request: Record<string, never>; result: { tick: string; instructions: string[] } };
  fingerprint: { request: Record<string, never>; result: EngineFingerprint };
  preview: {
    request: Record<string, never>;
    result: { ticksPerSecond: string; state: { frames: number } };
  };
  preview_state: {
    request: Record<string, never>;
    result: { ticksPerSecond: string; state: { frames: number } };
  };
  /** Stopped host preference, default zero; Exact always computes inline. */
  device_workers: {
    request: { count?: 0 | 2 };
    result: { count: 0 | 2; effective: 0 | 2 };
  };
  device_states: {
    request: { kind: string };
    result: { states: unknown[] };
  };
  memory_read: {
    request: { address: string; size: number };
    result: { address: string; size: number };
  };
  watch_pcs: {
    request: { addresses: number[] };
    result: { addresses: number[] };
  };
  profile: {
    request: { limit?: number };
    result: { ticks: number; instructions: number[]; pcEntries: unknown[]; failed: boolean };
  };
  error_report: {
    request: Record<string, never>;
    result: {
      report: {
        bus: { devices: { kind: string; state: unknown }[] };
        [key: string]: unknown;
      };
      snapshot?: { offset: number; length: number };
    };
  };
}
export class UnsupportedEmulatorOperation extends Error {}
export class GuestExecutionError extends Error {}

/** Owns allocations; every returned pixel span borrows the current Wasm heap. */
export class EmulatorSession {
  readonly timings = new HostTimings();
  readonly module: EmulatorModule;
  private readonly onTrap?: (error: unknown) => void;
  private scratch: number;
  private handle = 0;
  constructor(module: EmulatorModule, onTrap?: (error: unknown) => void) {
    this.module = module;
    this.onTrap = onTrap;
    this.scratch = this.call(() => module._camera_alloc(32));
    if (!this.scratch || this.scratch % 8) throw Error('Invalid Wasm descriptor allocation.');
  }
  private call<T>(operation: () => T): T {
    try {
      return operation();
    } catch (error) {
      this.onTrap?.(error);
      throw error;
    }
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
    const pointer = this.call(() => this.module._camera_alloc(bytes.length));
    if (!pointer) throw Error('Wasm allocation failed.');
    try {
      this.module.HEAPU8.set(bytes, pointer);
      return run(pointer, bytes.length);
    } finally {
      this.call(() => this.module._camera_free(pointer, bytes.length));
    }
  }
  private json<T>(value: unknown, run: (pointer: number, length: number) => T) {
    return this.upload(new TextEncoder().encode(JSON.stringify(value)), run);
  }
  private error(handle = this.handle) {
    if (this.call(() => this.module._camera_error(handle, this.scratch)) !== 0)
      return 'Wasm error unavailable.';
    return new TextDecoder().decode(this.span(this.scratch).slice());
  }
  private check(code: number) {
    if (code !== 0) throw Error(this.error());
  }
  create(config: unknown, dat: Uint8Array, construction: EmulatorManifest['construction']) {
    if (this.handle) throw Error('Session already created.');
    const create =
      construction === 'resolved' ? this.module._camera_create : this.module._camera_session_create;
    if (!create)
      throw new UnsupportedEmulatorOperation('Missing C ABI export: camera_session_create.');
    this.handle = this.json(config, (p, n) =>
      this.upload(dat, (d, l) => this.call(() => create(p, n, d, l))),
    );
    if (!this.handle) throw Error(this.error(0));
  }
  status() {
    this.check(this.call(() => this.module._camera_status(this.handle, this.scratch)));
    const v = this.view();
    return {
      ticks: v.getBigUint64(this.scratch, true),
      instructions: v.getBigUint64(this.scratch + 8, true),
      failed: v.getUint32(this.scratch + 16, true) !== 0,
    };
  }
  run(until: bigint) {
    const result = this.timings.measure('run', () =>
      this.call(() => this.module._camera_run_until(this.handle, until)),
    );
    if (result !== 0) throw new GuestExecutionError(this.error());
  }
  input(key: string, pressed: boolean) {
    const { ticks } = this.status();
    this.jsonString(key, (p, n) =>
      this.check(
        this.call(() => this.module._camera_input(this.handle, p, n, Number(pressed), ticks)),
      ),
    );
  }
  private jsonString<T>(value: string, run: (p: number, n: number) => T) {
    return this.upload(new TextEncoder().encode(value), run);
  }
  frame(output: number) {
    const code = this.timings.measure('frame', () =>
      this.call(() => this.module._camera_frame(this.handle, output, this.scratch)),
    );
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
    bytes: Uint8Array = new Uint8Array(),
  ): { result: SessionCommands[K]['result']; bytes: Uint8Array<ArrayBuffer> } {
    return this.timings.measure(`command.${action}`, () =>
      this.executeCommand(action, options, bytes),
    );
  }
  private executeCommand<K extends keyof SessionCommands>(
    action: K,
    options: SessionCommands[K]['request'],
    bytes: Uint8Array,
  ): { result: SessionCommands[K]['result']; bytes: Uint8Array<ArrayBuffer> } {
    const command = this.module._camera_command;
    if (!command)
      throw new UnsupportedEmulatorOperation(`Missing C ABI export: camera_command (${action}).`);
    return this.json({ action, options }, (p, n) =>
      this.upload(bytes, (b, l) => {
        const code = this.call(() => command(this.handle, p, n, b, l, this.scratch));
        if (code !== 0) {
          const message = this.error();
          if (this.status().failed) throw new GuestExecutionError(message);
          throw Error(message);
        }
        // Decode before freeing uploads: free is allowed to invalidate borrowed results.
        const result = JSON.parse(
          new TextDecoder().decode(this.span(this.scratch).slice()),
        ) as SessionCommands[K]['result'];
        return { result, bytes: this.span(this.scratch + 8).slice() };
      }),
    );
  }
  scene(scene: { width: number; height: number; rgb: Uint8Array } | null) {
    const { _camera_scene_buffer: reserve, _camera_scene_commit: commit } = this.module;
    if (!reserve || !commit)
      throw new UnsupportedEmulatorOperation(
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
      this.check(this.call(() => reserve(this.handle, width, height, this.scratch)));
      const target = this.span(this.scratch);
      if (target.length !== rgb.length) throw Error('Invalid Wasm scene span.');
      target.set(rgb);
    }
    this.check(this.call(() => commit(this.handle, Number(scene !== null))));
  }
  dispose() {
    try {
      if (this.handle) this.check(this.call(() => this.module._camera_destroy(this.handle)));
    } finally {
      this.handle = 0;
      this.call(() => this.module._camera_free(this.scratch, 32));
      this.scratch = 0;
    }
  }
}
