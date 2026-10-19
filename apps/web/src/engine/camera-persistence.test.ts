import { testBoot, testConfig, fakeWorker, fakeRuntime, type TestMessage } from './test-fixtures';
import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { WorkerEngine } from './worker-engine';
import { WorkerRecovery } from './recovery-worker';
function setup() {
  Object.assign(globalThis, { document: { baseURI: 'https://example.invalid/' } });
  Object.assign(globalThis, { window: { setTimeout: () => 0 } });
  globalThis.fetch = async () =>
    new Response(
      JSON.stringify({
        inputs: [
          {
            id: 'power',
            kind: 'switch',
            binding: { type: 'session_power', default_position: 'on' },
          },
        ],
      }),
    );
  const sent: TestMessage[] = [],
    errors: Error[] = [];
  const worker = {
    onmessage: (_event: { data: unknown }) => {},
    onerror: (_event: {
      message: string;
      filename?: string;
      lineno?: number;
      colno?: number;
    }) => {},
    terminated: false,
    postMessage(data: TestMessage) {
      sent.push(data);
      if (data.type === 'boot')
        queueMicrotask(() =>
          this.reply({
            type: 'ready',
            state: { virtual_ticks: 0, power: { on: false }, input_positions: { power: 'off' } },
          }),
        );
      if (data.type === 'sensor')
        queueMicrotask(() => this.reply({ type: 'sensor_ready', id: data.id }));
      if (data.type === 'restart')
        queueMicrotask(() =>
          this.reply({
            type: 'restarted',
            id: data.id,
            state: { virtual_ticks: 0, power: { on: true } },
          }),
        );
    },
    reply(data: unknown) {
      this.onmessage({ data });
    },
    terminate() {},
  };
  const engine = new WorkerEngine(
    () => fakeWorker(worker),
    (error) => errors.push(error),
  );
  return { engine, worker, sent, errors };
}
const config = {
  firmwareId: 'camera',
  inputs: 'inputs.json',
  bundle: 'runtime/',
  boot: { ...testBoot, sceneInput: true },
  camera: { id: 'owned-camera', revision: 3, modEnabled: false },
};
test('owned identity reaches boot; restored OFF defers scene input and powers on with the ON position', async () => {
  const { engine, sent } = setup();
  try {
    const inputs = await engine.boot(new ArrayBuffer(1), config, () => {});
    assert.deepEqual(sent.find((message) => message.type === 'boot')!.config.camera, config.camera);
    assert.equal(inputs[0].initial, 'off');
    await engine.setScene({ width: 1, height: 1, rgb: new Uint8Array([1, 2, 3]) });
    assert.equal(
      sent.some((message) => message.type === 'sensor'),
      false,
    );
    await engine.power(true);
    assert.equal(sent.find((message) => message.type === 'restart')!.initialPositions.power, 'on');
    assert.equal(sent.at(-1)!.type, 'sensor');
  } finally {
    engine.dispose();
  }
});
test('quota failure rejects an explicit save and publishes its status without stopping execution', async () => {
  const { engine, worker, sent, errors } = setup();
  const events: { savedAt?: number; revision?: number; error?: string }[] = [],
    stop = engine.cameraSaves((event) => events.push(event));
  try {
    await engine.boot(new ArrayBuffer(1), config, () => {});
    const save = engine.saveCamera(),
      request = sent.at(-1)!;
    assert.equal(request.type, 'save_camera');
    worker.reply({ type: 'camera_saved', error: 'Quota exceeded' });
    worker.reply({ type: 'error', id: request.id, error: 'Quota exceeded' });
    await assert.rejects(save, /Quota exceeded/);
    assert.equal(errors.length, 0);
    assert.deepEqual([...events], [{ type: 'camera_saved', error: 'Quota exceeded' }]);
    const next = engine.saveCamera(),
      second = sent.at(-1)!;
    worker.reply({ type: 'camera_saved', savedAt: 123, revision: 4 });
    worker.reply({ type: 'save_camera_result', id: second.id });
    await next;
    assert.equal(events.at(-1)!.revision, 4);
    stop();
    worker.reply({ type: 'camera_saved', revision: 5 });
    assert.equal(events.length, 2);
  } finally {
    engine.dispose();
  }
});
test('an unsafe live boundary cannot reuse an older snapshot with newer durable storage', () => {
  const recovery = new WorkerRecovery(),
    calls: string[] = [];
  const py = {
    runPython(source: string) {
      calls.push(source);
      return false;
    },
    FS: {
      readFile() {
        throw Error('Must not export a stale point');
      },
    },
  };
  assert.equal(recovery.ownedSnapshot(fakeRuntime(py)), undefined);
  assert.deepEqual(calls, ['browser_history.capture(session, force=True)']);
});
test('a rejected owned snapshot reports the reason and releases its proxy for a durable cold boot', async () => {
  const recovery = new WorkerRecovery(),
    released: string[] = [];
  const py = {
    toPy() {
      return {
        destroy() {
          released.push('proxy');
        },
      };
    },
    globals: {
      set() {},
      delete(name: string) {
        released.push(name);
      },
    },
    runPython(source: string) {
      if (source.includes('restore_session')) throw Error('Snapshot fingerprint differs');
      return '{}';
    },
  };
  assert.equal(await recovery.restoreOwned(fakeRuntime(py), new ArrayBuffer(1)), false);
  assert.equal(recovery.mode, 'cold');
  assert.match(recovery.status(fakeRuntime(py)).cacheFailure!, /Snapshot fingerprint differs/);
  assert.deepEqual(released, ['browser_checkpoint', 'proxy']);
});
