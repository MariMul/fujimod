import type { CameraModule, SessionState } from './wasm-abi';

/** C ABI double for transport/lifetime tests. Never loaded by an engine worker. */
export function testCameraModule(shared = false) {
  const memory = new WebAssembly.Memory({ initial: 2, maximum: 4, shared });
  const allocations = new Map<number, number>();
  let cursor = 64,
    ticks = 0n,
    failed = false;
  const calls: { action: string; options: Record<string, unknown>; bytes: number[] }[] = [];
  const state: SessionState = {
    inputs: [{ id: 'menu', label: 'Menu', kind: 'button', supported: true }],
    positions: { power: 'on' },
    connections: [],
    poweredOn: true,
    card: { supported: true, present: true, busy: false, revision: 1, capacity_bytes: 1000 },
  };
  const bytes = (p: number, n: number) => module.HEAPU8.subarray(p, p + n);
  const descriptor = (out: number, data: Uint8Array) => {
    const pointer = 70000;
    module.HEAPU8.set(data, pointer);
    const v = new DataView(memory.buffer);
    v.setUint32(out, pointer, true);
    v.setUint32(out + 4, data.length, true);
  };
  let createError = '',
    commandError = '',
    frameEnabled = false;
  const module: CameraModule = {
    get HEAPU8() {
      return new Uint8Array(memory.buffer);
    },
    _camera_alloc(n) {
      const p = cursor;
      cursor += Math.ceil(n / 8) * 8;
      allocations.set(p, n);
      return p;
    },
    _camera_free(p, n) {
      if (allocations.get(p) !== n) throw Error('Invalid free');
      allocations.delete(p);
    },
    _camera_create() {
      return createError ? 0 : 1;
    },
    _camera_session_create() {
      return createError ? 0 : 1;
    },
    _camera_destroy() {
      return 0;
    },
    _camera_run_until(_h, until) {
      if (failed) return -1;
      ticks = until;
      return 0;
    },
    _camera_input(_h, p, n, pressed, at) {
      calls.push({
        action: 'input',
        options: { key: new TextDecoder().decode(bytes(p, n)), pressed, at },
        bytes: [],
      });
      return 0;
    },
    _camera_status(_h, out) {
      const v = new DataView(memory.buffer);
      v.setBigUint64(out, ticks, true);
      v.setBigUint64(out + 8, ticks * 2n, true);
      v.setUint32(out + 16, Number(failed), true);
      return 0;
    },
    _camera_frame(_h, _output, out) {
      if (!frameEnabled) return 1;
      descriptor(out, new Uint8Array([10, 20, 30, 255]));
      const v = new DataView(memory.buffer);
      v.setUint32(out + 8, 1, true);
      v.setUint32(out + 12, 1, true);
      v.setUint32(out + 16, 0, true);
      v.setBigUint64(out + 24, ticks, true);
      return 0;
    },
    _camera_error(_h, out) {
      descriptor(
        out,
        new TextEncoder().encode(createError || commandError || 'unmapped guest load'),
      );
      return 0;
    },
    _camera_command(_h, p, n, b, l, out) {
      const request = JSON.parse(new TextDecoder().decode(bytes(p, n))) as {
        action: string;
        options: Record<string, unknown>;
      };
      calls.push({ ...request, bytes: [...bytes(b, l)] });
      if (commandError) return -1;
      let result: unknown = state,
        binary = new Uint8Array();
      switch (request.action) {
        case 'power':
          state.poweredOn = Boolean(request.options.on);
          ticks = 0n;
          break;
        case 'power_cycle':
        case 'restart_card':
          state.poweredOn = true;
          ticks = 0n;
          break;
        case 'card_state':
          result = state.card;
          break;
        case 'card_tree':
          result = { ...state.card, entries: [] };
          break;
        case 'card_read':
          result = {};
          binary = new Uint8Array([4, 5, 6]);
          break;
        case 'preview':
        case 'preview_state':
          result = { ticksPerSecond: '12000000', state: { frames: Number(ticks / 1200000n) } };
          break;
        case 'checkpoint':
          result = {
            id: String(ticks),
            tick: String(ticks),
            instructions: [String(ticks * 2n)],
            retainedIds: [String(ticks)],
          };
          break;
        case 'error_report':
          result = {
            report: {
              bus: {
                devices: [
                  { kind: 'external-image-frames', state: { frames: Number(ticks / 400000n) } },
                ],
              },
            },
          };
          break;
        case 'restore':
          ticks = BigInt(String(request.options.id));
          failed = false;
          result = { tick: String(ticks), instructions: [String(ticks * 2n)] };
          break;
        case 'history':
          result = { format: 'session-input-log-cbor', schema: 1, entries: 1 };
          binary = new Uint8Array([0x81, 0x01]);
          break;
        case 'replay':
          result = { tick: String(ticks), instructions: [String(ticks * 2n)] };
          break;
        case 'fingerprint':
          result = { schema: 1, sha256: 'a'.repeat(64) };
          break;
        case 'rollback':
          failed = false;
          ticks = 10n;
          break;
        case 'profile':
          result = { measured: true };
          break;
        case 'save':
          result = {
            durable: { offset: 0, length: 2 },
            positions: state.positions,
            poweredOn: true,
          };
          binary = new Uint8Array([1, 2]);
          break;
      }
      descriptor(out, new TextEncoder().encode(JSON.stringify(result)));
      module.HEAPU8.set(binary, 80000);
      const v = new DataView(memory.buffer);
      v.setUint32(out + 8, 80000, true);
      v.setUint32(out + 12, binary.length, true);
      return 0;
    },
    _camera_scene_buffer(_h, w, h, out) {
      descriptor(out, new Uint8Array(w * h * 3));
      return 0;
    },
    _camera_scene_commit(_h, enabled) {
      calls.push({ action: 'scene', options: { enabled }, bytes: [...bytes(70000, 3)] });
      return 0;
    },
  };
  return {
    module,
    memory,
    allocations,
    calls,
    state,
    createError(value: string) {
      createError = value;
    },
    commandError(value: string) {
      commandError = value;
    },
    fail() {
      failed = true;
    },
    frame() {
      frameEnabled = true;
    },
  };
}
