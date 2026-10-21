import { readFile } from 'node:fs/promises';
import { beforeAll, expect, test, vi } from 'vite-plus/test';
import { initialize, usb, type DeviceState } from 'fujimod';
import type { Callbacks, UsbExporter } from '../../../../crates/usb-proxy/web/types.ts';
import { installWebUsb } from '../../../../bindings/javascript/tests/webusb-fixture.ts';
import {
  createUsbCameraModel,
  type ModelDevice,
  type ModelSession,
  type ModelStream,
  type UsbCameraServices,
} from './usb-camera-model';
import { Layer, Level } from './log';
beforeAll(async () => {
  await initialize({
    module_or_path: await readFile(
      new URL('../../../../bindings/javascript/pkg/fujimod_bg.wasm', import.meta.url),
    ),
  });
});
class Stream<T> implements ModelStream<T> {
  values: T[] = [];
  waiting?: (value: T | undefined) => void;
  ended = false;
  next(): Promise<T | undefined> {
    if (this.values.length) return Promise.resolve(this.values.shift());
    if (this.ended) return Promise.resolve(undefined);
    return new Promise((resolve) => {
      this.waiting = resolve;
    });
  }
  push(value: T) {
    if (this.waiting) {
      const resolve = this.waiting;
      this.waiting = undefined;
      resolve(value);
    } else this.values.push(value);
  }
  close() {
    this.ended = true;
    this.waiting?.(undefined);
  }
  free() {}
}
type Exchange = NonNullable<Awaited<ReturnType<ReturnType<ModelDevice['watchLog']>['next']>>>;
class CameraDevice implements ModelDevice {
  productName = 'Fixture';
  manufacturer = 'Maker';
  serialNumber = 'serial';
  status: DeviceState;
  states = new Stream<DeviceState>();
  logs = new Stream<Exchange>();
  acquire = vi.fn();
  closed = vi.fn();
  freed = vi.fn();
  releases = vi.fn();
  failRead = false;
  failClose = false;
  constructor(readonly id: string) {
    this.status = {
      id,
      present: true,
      open: true,
      sharing: false,
      sessionOwner: null,
      fenced: false,
    };
    this.states.push(this.status);
  }
  update(patch: Partial<DeviceState>) {
    this.status = { ...this.status, ...patch };
    this.states.push(this.status);
  }
  async open() {
    this.update({ open: true, fenced: false });
  }
  async close() {
    this.closed();
  }
  free() {
    this.freed();
  }
  async interfaces() {
    return [new usb.InterfaceSelection(1, 2, 3)];
  }
  watch() {
    return this.states;
  }
  watchLog() {
    return this.logs;
  }
  emit(kind = 0xd0, consoleData: Uint8Array | null = null, overrides: Partial<Exchange> = {}) {
    this.logs.push({
      deviceId: this.id,
      holder: this.status.sessionOwner ?? { kind: 'remote', controller: 'peer' },
      sequence: 1,
      timestampMs: 100,
      kind,
      session: 1,
      operation: 'Read memory',
      command: 0x200001,
      function: 2,
      address: 0x1234,
      selector: null,
      page: null,
      length: 4,
      requested: 4,
      transferred: 4,
      envelope: new Uint8Array(12),
      data: new Uint8Array(4),
      error: null,
      truncated: false,
      consoleData,
      ...overrides,
    });
  }
  share(): UsbExporter {
    this.update({ sharing: true });
    return {
      inventoryJson: '[]',
      execute: async () => new Uint8Array(),
      busy: () => false,
      owns: () => false,
      fenced: () => false,
      abandon: () => false,
      stopSharing: () => this.stopSharing(),
      shutdown: async () => {},
      free() {},
    };
  }
  stopSharing() {
    this.update({ sharing: false });
  }
  async acquireSession(
    _s: Parameters<ModelDevice['acquireSession']>[0],
    _l: Parameters<ModelDevice['acquireSession']>[1],
    _o: Parameters<ModelDevice['acquireSession']>[2],
    tool?: string,
  ): Promise<ModelSession> {
    this.acquire(tool);
    this.update({ sessionOwner: { kind: 'local', tool } });
    return {
      state: usb.SessionState.Ready,
      close: async () => {
        this.releases();
        this.update({ sessionOwner: null });
        if (this.failClose) throw Error('close failed');
        return new Uint8Array();
      },
      free() {},
      abandon: () => this.update({ fenced: true }),
      deviceInfo: async () => ({ text: 'Identity', raw: new Uint8Array(), free() {} }),
      configurationGeometry: async () => ({ capacity: 16, pageBytes: 4, free() {} }),
      readMemory: async () => {
        if (this.failRead) throw Error('read failed');
        this.emit();
        this.emit(0xd1);
        return Uint8Array.of(1, 2, 3, 4);
      },
      readConfiguration: async () => Uint8Array.of(5),
      submitConsole: async (data) => {
        this.emit(0xd0, data);
        this.emit(0xd1);
        return new Uint8Array();
      },
      pollConsole: async () => ({
        progress: usb.ConsoleProgress.Pending,
        status: 0,
        output: new Uint8Array(),
        free() {},
      }),
      armResetGuard: async () => {
        throw Error('unused');
      },
      enterDebugMode: async () => {
        throw Error('unused');
      },
      leaveDebugMode: async () => {
        throw Error('unused');
      },
    };
  }
}
function fixture(
  devices: CameraDevice[] = [new CameraDevice('one')],
  overrides: Partial<UsbCameraServices> = {},
) {
  const callbacks = new Map<string, Callbacks>();
  let picker: ModelDevice | undefined;
  let remote: ModelDevice[] = [];
  let now = 100;
  const pause = vi.fn(async (_ms: number) => {});
  const services: Partial<UsbCameraServices> = {
    initialize: async () => {},
    discover: async () => devices,
    requestDevice: async () => picker,
    discoverProxy: async () => remote,
    now: () => now,
    pause,
    share: async (device, cb) => {
      callbacks.set(device.id, cb);
      const exporter = device.share();
      return {
        link: `https://example.com/#${device.id}`,
        host: {
          stop() {
            exporter.stopSharing();
            exporter.free();
          },
          cameraLost() {},
          cameraReturned(exporter) {
            exporter.free();
          },
        },
      };
    },
  };
  return {
    model: createUsbCameraModel({ ...services, ...overrides }),
    devices,
    callbacks,
    pause,
    picker(device: ModelDevice) {
      picker = device;
    },
    remote(device: ModelDevice) {
      remote = [device];
    },
    now(value: number) {
      now = value;
    },
  };
}
async function idle(model: ReturnType<typeof createUsbCameraModel>, count = 1) {
  await vi.waitFor(() => {
    expect(model.cameras()).toHaveLength(count);
    for (const camera of model.cameras()) {
      expect(camera.session).toBeUndefined();
      expect(camera.identity).toBeDefined();
    }
  });
}

test('multiple devices: start share → local read → stop share → local read', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    const d = f.devices[0];
    expect(d.acquire).toHaveBeenCalledExactlyOnceWith('identity');
    const other = new CameraDevice('two');
    f.picker(other);
    await f.model.connectUsb();
    await idle(f.model, 2);
    await f.model.setSharing(d.id, true);
    await f.model.withSession(d.id, 'memory', (s) => s.readMemory(0, 4));
    await f.model.setSharing(d.id, false);
    await f.model.withSession(d.id, 'memory', (s) => s.readMemory(0, 4));
    expect(f.model.cameras()[0]).toMatchObject({ present: true, uncertain: false });
    expect(f.model.cameras()[0].sharing).toBeUndefined();
    expect(d.closed).not.toHaveBeenCalled();
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(2),
    );
  } finally {
    f.model.dispose();
  }
});
test('held and uncertain devices refuse before acquire; other cameras remain usable', async () => {
  const f = fixture([new CameraDevice('one'), new CameraDevice('two')]);
  try {
    await idle(f.model, 2);
    const d = f.devices[0];
    for (const patch of [
      { fenced: true },
      { fenced: false, sessionOwner: { kind: 'remote' as const, controller: 'peer' } },
      { present: false },
    ]) {
      d.update(patch);
      await Promise.resolve();
      const calls = d.acquire.mock.calls.length;
      await expect(f.model.withSession(d.id, 'read', (s) => s.readMemory(0, 1))).rejects.toThrow();
      expect(d.acquire).toHaveBeenCalledTimes(calls);
    }
    await f.model.withSession('two', 'read', (s) => s.readMemory(0, 1));
    let release!: () => void;
    const run = f.model.withSession(
      'two',
      'hold',
      async () =>
        new Promise<void>((r) => {
          release = r;
        }),
    );
    await vi.waitFor(() => expect(release).toBeTypeOf('function'));
    await expect(f.model.withSession('two', 'second', async () => {})).rejects.toThrow(/held/);
    release();
    await run;
  } finally {
    f.model.dispose();
  }
});
test('replug preserves id, rereads identity, and resolves unplugged problem', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    const d = f.devices[0];
    await f.model.setSharing(d.id, true);
    f.now(500);
    d.update({ present: false });
    await vi.waitFor(() => expect(f.model.cameras()[0].goneAt).toBe(500));
    expect(f.model.cameras()[0].problem).toMatch(/disconnected/);
    d.update({ present: true });
    await vi.waitFor(() => expect(d.acquire).toHaveBeenCalledTimes(2));
    await idle(f.model);
    expect(f.model.cameras()[0].id).toBe(d.id);
    expect(f.model.cameras()[0].goneAt).toBeUndefined();
    expect(f.model.cameras()[0].problem).toBeUndefined();
  } finally {
    f.model.dispose();
  }
});
test('always closes and preserves primary operation failure', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    const d = f.devices[0];
    d.failRead = true;
    d.failClose = true;
    await expect(f.model.withSession(d.id, 'read', (s) => s.readMemory(0, 1))).rejects.toThrow(
      'read failed',
    );
    expect(d.releases).toHaveBeenCalledTimes(2);
  } finally {
    f.model.dispose();
  }
});
test('remote callbacks update activity/problems and await owner countdown', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    const d = f.devices[0];
    await f.model.setSharing(d.id, true);
    const cb = f.callbacks.get(d.id)!;
    cb.connected!('peer', 'job');
    d.update({ sessionOwner: { kind: 'remote', controller: 'peer' } });
    cb.activity!('peer', 'Read RAM');
    cb.progress!('peer', { label: 'Bytes', done: 4, total: 16 });
    await vi.waitFor(() =>
      expect(f.model.cameras()[0].remoteActivity).toEqual({
        controller: 'peer',
        title: 'Read RAM',
        progress: { label: 'Bytes', done: 4, total: 16 },
      }),
    );
    expect(f.model.cameras()[0].sharing?.peers.get('peer')).toBe('job');
    cb.attention!('peer', 'Check camera');
    expect(f.model.cameras()[0].problem).toBe('Check camera');
    f.model.dismissProblem(d.id);
    expect(f.model.cameras()[0].problem).toBeUndefined();
    let resolve!: () => void;
    const handler = vi.fn(
      () =>
        new Promise<void>((r) => {
          resolve = r;
        }),
    );
    const unsubscribe = f.model.onOwnerCountdown(handler);
    let done = false;
    const waiting = cb.countdown!('peer', 'Look now').then(() => {
      done = true;
    });
    expect(handler).toHaveBeenCalledWith(d.id, 'Look now');
    await Promise.resolve();
    expect(done).toBe(false);
    resolve();
    await waiting;
    unsubscribe();
    await expect(cb.countdown!('peer', 'Again')).rejects.toThrow(/unavailable/);
    cb.disconnected!('peer', 'done');
    expect(f.model.cameras()[0].remoteActivity).toBeUndefined();
  } finally {
    f.model.dispose();
  }
});
test('Rust console events log local and remote text; empty poll waits', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    const d = f.devices[0];
    await f.model.withSession(d.id, 'console', async (s) => {
      await s.submitConsole(new TextEncoder().encode('help'));
      expect((await s.pollConsole()).complete).toBe(false);
    });
    d.update({ sessionOwner: { kind: 'remote', controller: 'peer' } });
    d.emit(0xd1, new TextEncoder().encode('hello\n'));
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Console)).toHaveLength(2),
    );
    expect(
      f.model
        .log()
        .filter((row) => row.layer === Layer.Console)
        .map((row) => [row.camera, row.holder, row.message]),
    ).toEqual([
      [d.id, { kind: 'local', tool: 'console' }, 'help'],
      [d.id, { kind: 'remote', controller: 'peer', reason: '' }, 'hello\n'],
    ]);
    expect(f.pause).toHaveBeenCalledExactlyOnceWith(50);
  } finally {
    f.model.dispose();
  }
});
test('remote model uses binding events and never calls USB-only sharing on removal', async () => {
  const f = fixture([]);
  try {
    const remote = new CameraDevice('remote');
    remote.stopSharing = () => {
      throw Error('USB only');
    };
    f.remote(remote);
    await f.model.connectRemote('https://example.com/share');
    await idle(f.model);
    expect(f.model.cameras()[0].transport).toBe('remote');
    await expect(f.model.setSharing(remote.id, true)).rejects.toThrow(/remote/);
    await f.model.withSession(remote.id, 'read', (s) => s.readMemory(0, 1));
    await f.model.remove(remote.id);
    expect(f.model.cameras()).toHaveLength(0);
    expect(remote.closed).toHaveBeenCalledOnce();
    expect(remote.freed).toHaveBeenCalledOnce();
  } finally {
    f.model.dispose();
  }
});
test('production WebUSB owner keeps one open/claim through sharing and reads', async () => {
  const wire = installWebUsb();
  wire.state.interfaceDelay = 20;
  wire.iface.alternates[0].endpoints = [
    { endpointNumber: 2, direction: 'in', type: 'bulk', packetSize: 64 },
    { endpointNumber: 1, direction: 'out', type: 'bulk', packetSize: 64 },
  ];
  wire.iface.alternate = wire.iface.alternates[0];
  const transfer = wire.device.transferOut.bind(wire.device);
  wire.device.transferOut = async (endpoint, raw) => {
    const request = new Uint8Array(raw),
      view = new DataView(request.buffer, request.byteOffset, request.byteLength);
    if (wire.state.kind === 0xd0 && view.getUint32(0, true) === 0x150000) {
      const text = 'SYNTHETIC\0',
        response = new Uint8Array(16 + text.length * 2),
        reply = new DataView(response.buffer);
      reply.setUint32(0, 0x150000, true);
      Array.from(text).forEach((ch, i) => reply.setUint16(16 + i * 2, ch.charCodeAt(0), true));
      wire.state.pending = response;
      wire.log.push(['out', endpoint, request]);
      return { status: 'ok', bytesWritten: request.length };
    }
    return transfer(endpoint, raw);
  };
  const model = createUsbCameraModel({
    share: async (device) => {
      const exporter = device.share();
      return {
        link: 'https://example.com/share',
        host: {
          stop() {
            exporter.stopSharing();
            exporter.free();
          },
          cameraLost() {},
          cameraReturned(exporter) {
            exporter.free();
          },
        },
      };
    },
  });
  try {
    await idle(model);
    const id = model.cameras()[0].id;
    model.clearLog();
    await model.setSharing(id, true);
    await model.withSession(id, 'memory', (s) => s.readMemory(0x1000, 4));
    await model.setSharing(id, false);
    await model.withSession(id, 'memory', (s) => s.readMemory(0x1000, 4));
    await vi.waitFor(() =>
      expect(model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(6),
    );
    expect(wire.log.filter(([kind]) => kind === 'open')).toHaveLength(1);
    expect(wire.log.filter(([kind]) => kind === 'claim')).toHaveLength(1);
    expect(
      wire.log.filter(([kind]) => ['release', 'close', 'refused'].includes(kind)),
    ).toHaveLength(0);
    for (const row of model.log().filter((row) => row.layer === Layer.Ff80))
      expect(row.holder).toEqual({ kind: 'local', tool: 'memory' });
    await model.remove(id);
    expect(wire.state.interfaceChanging).toBe(false);
  } finally {
    model.dispose();
    wire.restore();
  }
});

test('stopping a pending share and then restarting creates only the current attachment', async () => {
  const device = new CameraDevice('one');
  const ready: (() => void)[] = [];
  const stopped = vi.fn();
  const f = fixture([device], {
    share: async (device) => {
      const exporter = device.share();
      await new Promise<void>((resolve) => ready.push(resolve));
      return {
        link: 'https://example.com/share',
        host: {
          stop() {
            stopped();
            exporter.stopSharing();
            exporter.free();
          },
          cameraLost() {},
          cameraReturned(exporter) {
            exporter.free();
          },
        },
      };
    },
  });
  try {
    await idle(f.model);
    const first = f.model.setSharing(device.id, true);
    await f.model.setSharing(device.id, false);
    const next = f.model.setSharing(device.id, true);
    ready[0]();
    await first;
    await vi.waitFor(() => expect(ready).toHaveLength(2));
    ready[1]();
    await next;
    expect(stopped).toHaveBeenCalledOnce();
    expect(f.model.cameras()[0].sharing).toBeDefined();
    expect(f.model.cameras()[0].present).toBe(true);
  } finally {
    f.model.dispose();
  }
});

test('one command row merges request/reply totals and payloads, with errors on either phase', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    f.model.clearLog();
    const d = f.devices[0];
    d.update({ sessionOwner: { kind: 'local', tool: 'configuration' } });
    const request = {
      operation: 'Read configuration',
      address: 0x300,
      length: 256,
      data: Uint8Array.of(1, 2, 3),
      transferred: 3,
      envelope: new Uint8Array(12).fill(0xd0),
    };
    d.emit(0xd0, null, request);
    await Promise.resolve();
    expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(0);
    d.emit(0xd1, null, {
      operation: 'Reply header',
      address: 0,
      length: 2,
      data: Uint8Array.of(4, 5),
      transferred: 2,
    });
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(1),
    );
    const row = f.model.log().find((row) => row.layer === Layer.Ff80)!;
    expect(row.message).toBe('Read configuration · 0x300 · 256 B');
    expect(row.sizes).toEqual({ sent: 27, received: 2 });
    expect(row.data).toEqual(Uint8Array.of(4, 5));
    expect(row.detail).toContain('Request payload: 01 02 03');
    expect(row.holder).toEqual({ kind: 'local', tool: 'configuration' });
    expect(row.level).toBe(Level.Debug);
    d.emit(0xd0, null, { ...request, error: 'request stall' });
    d.emit(0xd0, null, request);
    d.emit(0xd1, null, { data: Uint8Array.of(9), transferred: 1, error: 'reply disconnected' });
    d.emit(0xc0, null, {
      operation: 'Open session',
      address: null,
      length: null,
      data: new Uint8Array(8),
      transferred: 8,
    });
    d.emit(0xc4, null, {
      operation: 'Close session',
      address: null,
      length: null,
      data: new Uint8Array(2),
      transferred: 2,
    });
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(5),
    );
    const rows = f.model.log().filter((row) => row.layer === Layer.Ff80);
    expect(rows[1].level).toBe(Level.Error);
    expect(rows[1].detail).toContain('request stall');
    expect(rows[1].data).toEqual(new Uint8Array());
    expect(rows[2].level).toBe(Level.Error);
    expect(rows[2].detail).toContain('reply disconnected');
    expect(rows[2].sizes).toEqual({ sent: 27, received: 1 });
    expect(rows[2].data).toEqual(Uint8Array.of(9));
    expect(rows.slice(3).map((row) => row.message)).toEqual(['Open session', 'Close session']);
    expect(rows.slice(3).map((row) => row.sizes)).toEqual([
      { sent: 12, received: 8 },
      { sent: 12, received: 2 },
    ]);
    d.emit(0xe0, null, { operation: 'Write data', data: Uint8Array.of(7, 8), transferred: 2 });
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(6),
    );
    const upload = f.model
      .log()
      .filter((row) => row.layer === Layer.Ff80)
      .at(-1)!;
    expect(upload.data).toEqual(Uint8Array.of(7, 8));
    expect(upload.sizes).toEqual({ sent: 14, received: 0 });
  } finally {
    f.model.dispose();
  }
});

test('identity log rows name all seven wire selectors and decode their returned values', async () => {
  const f = fixture();
  try {
    await idle(f.model);
    f.model.clearLog();
    const d = f.devices[0];
    const identities = [
      'Vendor',
      'Manufacturer',
      'Product',
      'InternalVersion',
      'DeviceType',
      'Serial',
      'Framework',
    ];
    const response = (text: string) => {
      const data = new Uint8Array(16 + (text.length + 1) * 2),
        view = new DataView(data.buffer);
      view.setUint32(0, 0x150000, true);
      for (let i = 0; i < text.length; i++) view.setUint16(16 + i * 2, text.charCodeAt(i), true);
      return data;
    };
    for (const [index, name] of identities.entries()) {
      const data = response(name === 'Product' ? 'X100V' : `Value ${name}`);
      d.emit(0xd0, null, {
        command: 0x150000,
        operation: 'Device info',
        selector: 1 << index,
        address: null,
        length: null,
      });
      d.emit(0xd1, null, { command: 0x150000, selector: null, data, transferred: data.length });
    }
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(7),
    );
    expect(
      f.model
        .log()
        .filter((row) => row.layer === Layer.Ff80)
        .map((row) => row.message),
    ).toEqual(
      identities.map(
        (name) => `Device info · ${name} → ${name === 'Product' ? 'X100V' : `Value ${name}`}`,
      ),
    );
    const invalid = [
      response('X').slice(0, -2),
      response('\ud800'),
      new Uint8Array(8),
      response('Failed'),
    ];
    new DataView(invalid[3].buffer).setUint32(4, 1, true);
    for (const data of invalid) {
      d.emit(0xd0, null, {
        command: 0x150000,
        operation: 'Device info',
        selector: 4,
        address: null,
        length: null,
      });
      d.emit(0xd1, null, { data, transferred: data.length });
    }
    await vi.waitFor(() =>
      expect(f.model.log().filter((row) => row.layer === Layer.Ff80)).toHaveLength(11),
    );
    expect(
      f.model
        .log()
        .filter((row) => row.layer === Layer.Ff80)
        .slice(7)
        .map((row) => row.message),
    ).toEqual(Array(4).fill('Device info · Product'));
  } finally {
    f.model.dispose();
  }
});
