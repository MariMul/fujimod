import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { WasmSession, type WasmManifest } from './wasm-abi';
import { WasmWorkerRuntime } from './wasm-worker-runtime';
import { testCameraModule } from './wasm-test-module';
import type { WasmRequest, WasmWorkerEvent } from './wasm-protocol';
import { testConfig } from './test-fixtures';
import type { CameraRecord, CameraSave } from '../cameras/store';

function setup(options: { saved?: CameraSave; saveError?: string; shared?: boolean } = {}) {
  const fake = testCameraModule(options.shared),
    events: WasmWorkerEvent[] = [],
    saves: CameraSave[] = [];
  const manifest: WasmManifest = {
    schema: 1,
    abi: 1,
    module: '',
    wasm: '',
    construction: 'session',
    session: {},
    ticksPerSecond: 12000000,
    bootTicks: 0,
    output: 0,
  };
  const runtime = new WasmWorkerRuntime(
    (event) => {
      events.push(event);
      if (event.type === 'reply' || event.type === 'error') {
        pending.get(event.serial!)?.(event);
        pending.delete(event.serial!);
      }
    },
    async () => ({ session: new WasmSession(fake.module), manifest, identity: 'test' }),
    {
      async saved() {
        return options.saved;
      },
      async save(_id, revision, save) {
        if (options.saveError) throw Error(options.saveError);
        saves.push(save);
        return { revision: revision + 1, savedAt: 123 } as CameraRecord;
      },
    },
  );
  let serial = 0;
  const pending = new Map<number, (event: WasmWorkerEvent) => void>();
  const send = (request: WasmRequest) =>
    new Promise<WasmWorkerEvent>((resolve) => {
      const id = ++serial;
      pending.set(id, resolve);
      runtime.receive({ type: 'rpc', serial: id, request });
    });
  const config = {
    ...testConfig(),
    bundle: 'https://example.invalid/runtime/',
    camera: { id: 'camera', revision: 3, modEnabled: false },
    execution: { mode: 'replay' as const, clockSeconds2000: 0 },
  };
  return { ...fake, runtime, events, saves, send, config };
}
test('worker routes physical inputs, scene, card operations, restart, power, profile and rollback through the ABI', async () => {
  const f = setup();
  const boot = await f.send({
    action: 'boot',
    config: f.config,
    dat: new ArrayBuffer(1),
    files: [],
    seedIdentity: [],
  });
  assert.equal(boot.type, 'reply');
  const requests: WasmRequest[] = [
    { action: 'press', id: 'menu', stage: 'half', point: { x: 0.2, y: 0.5 } },
    { action: 'release', id: 'menu' },
    { action: 'set_position', id: 'dial', position: -1 },
    { action: 'scene', scene: { width: 1, height: 1, rgb: new Uint8Array([2, 3, 4]) } },
    { action: 'power', on: false },
    { action: 'power_cycle', inputs: [{ id: 'menu' }] },
    { action: 'card_state' },
    { action: 'card_tree', revision: 1 },
    { action: 'card_read', path: '/a', options: { revision: 1, maxBytes: 20 } },
    { action: 'card_insert', present: false },
    { action: 'card_format' },
    {
      action: 'restart_card',
      files: [
        { path: '/a', bytes: new Uint8Array([9, 8]) },
        { path: '/b', bytes: new Uint8Array([7]) },
      ],
    },
    { action: 'profile', limit: 5 },
    { action: 'rollback', seconds: 10 },
    {
      action: 'replay',
      steps: [
        { action: 'advance', ticks: 100 },
        { action: 'press', id: 'menu' },
        { action: 'release', id: 'menu' },
      ],
    },
  ];
  for (const request of requests)
    assert.equal((await f.send(request)).type, 'reply', request.action);
  assert.equal(f.calls.find((c) => c.action === 'press')!.options.stage, 'half');
  const restart = f.calls.find((c) => c.action === 'restart_card')!;
  assert.deepEqual(restart.options.files, [
    { path: '/a', offset: 0, length: 2 },
    { path: '/b', offset: 2, length: 1 },
  ]);
  assert.deepEqual(restart.bytes, [9, 8, 7]);
  assert.equal(f.calls.find((c) => c.action === 'card_read')!.options.revision, 1);
  assert(
    f.events.some((e) => e.type === 'metrics' && e.metrics.runtime?.ticksPerSecond === 12000000),
  );
});
test('camera saves commit matched durable/snapshot bytes with optimistic revisions', async () => {
  const f = setup();
  await f.send({
    action: 'boot',
    config: f.config,
    dat: new ArrayBuffer(1),
    files: [],
    seedIdentity: [],
  });
  assert.equal((await f.send({ action: 'save' })).type, 'reply');
  assert.equal((await f.send({ action: 'save' })).type, 'reply');
  assert.deepEqual([...new Uint8Array(f.saves[0]!.durable)], [1, 2]);
  assert.deepEqual([...new Uint8Array(f.saves[0]!.snapshot!)], [3, 4, 5]);
  assert.deepEqual(
    f.events.filter((e) => e.type === 'saved').map((e) => e.revision),
    [4, 5],
  );
});
test('quota rejection is an operation error, and guest execution failure supports rollback', async () => {
  const f = setup({ saveError: 'Quota exceeded' });
  await f.send({
    action: 'boot',
    config: f.config,
    dat: new ArrayBuffer(1),
    files: [],
    seedIdentity: [],
  });
  const failedSave = await f.send({ action: 'save' });
  assert(failedSave.type === 'error' && !failedSave.fault);
  assert(f.events.some((e) => e.type === 'saved' && e.error?.includes('Quota exceeded')));
  f.fail();
  const failure = await f.send({ action: 'replay', steps: [{ action: 'advance', ticks: 20 }] });
  assert(failure.type === 'error' && failure.fault?.kind === 'guest-fault');
  assert.equal((await f.send({ action: 'rollback', seconds: 10 })).type, 'reply');
  assert.equal(
    (await f.send({ action: 'replay', steps: [{ action: 'advance', ticks: 20 }] })).type,
    'reply',
  );
});
test('an incompatible snapshot restores durable storage without silently reusing stale execution state', async () => {
  const f = setup({
    saved: {
      durable: new Uint8Array([1, 2]).buffer,
      snapshot: new Uint8Array([9]).buffer,
      runtimeKey: 'different',
      positions: {},
      poweredOn: true,
      modEnabled: false,
    },
  });
  await f.send({
    action: 'boot',
    config: f.config,
    dat: new ArrayBuffer(1),
    files: [],
    seedIdentity: [],
  });
  const restore = f.calls.find((c) => c.action === 'restore')!;
  assert.deepEqual(restore.bytes, [1, 2]);
  assert.equal(restore.options.snapshot, undefined);
});

test('shared frame leases block queued guest mutations until presentation releases the view', async () => {
  const f = setup({ shared: true });
  f.frame();
  await f.send({
    action: 'boot',
    config: f.config,
    dat: new ArrayBuffer(1),
    files: [],
    seedIdentity: [],
  });
  const frame = f.events.find((event) => event.type === 'frame_view');
  assert(frame?.type === 'frame_view');
  const press = f.send({ action: 'press', id: 'menu' });
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  assert.equal(
    f.calls.some((call) => call.action === 'press'),
    false,
  );
  f.runtime.receive({ type: 'frame_release', lease: frame.lease + 1 });
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  assert.equal(
    f.calls.some((call) => call.action === 'press'),
    false,
  );
  f.runtime.receive({ type: 'frame_release', lease: frame.lease });
  assert.equal((await press).type, 'reply');
});

test('cold durable restore preserves physical state and merges replacement card files before boot', async () => {
  const f = setup({
    saved: {
      durable: new Uint8Array([1]).buffer,
      runtimeKey: 'old',
      positions: { power: 'off', dial: 'auto' },
      poweredOn: false,
      modEnabled: false,
    },
  });
  await f.send({
    action: 'boot',
    config: f.config,
    dat: new ArrayBuffer(1),
    files: [{ path: '/BOOT.BIN', bytes: new Uint8Array([8, 9]) }],
    seedIdentity: [],
  });
  const restore = f.calls.find((call) => call.action === 'restore')!;
  assert.deepEqual(restore.options.positions, { power: 'off', dial: 'auto' });
  assert.equal(restore.options.poweredOn, false);
  const actions = f.calls.map((call) => call.action);
  assert(actions.indexOf('restore') < actions.indexOf('restart_card'));
  assert.deepEqual(f.calls.find((call) => call.action === 'restart_card')!.bytes, [8, 9]);
});
