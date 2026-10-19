import { testBoot, testConfig, fakeWorker, fakeRuntime, type TestMessage } from './test-fixtures';
import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { DefaultSceneEngine } from './default-scene-engine.ts';

const config = {
  bundle: 'runtime/test/',
  inputs: 'runtime/test/inputs.json',
  firmwareId: 'test',
  boot: { ...testBoot, sceneInput: true },
};
const still = { width: 2, height: 1, rgb: new Uint8Array([20, 40, 60, 80, 100, 120]) };

function setup(loadDefault: () => Promise<import('./api').Scene> = async () => still) {
  Object.assign(globalThis, { document: { baseURI: 'https://example.invalid/' } });
  Object.assign(globalThis, { window: { setTimeout: () => 0 } });
  globalThis.fetch = async () => new Response(JSON.stringify({ inputs: [] }));
  const messages: TestMessage[] = [],
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
    postMessage(message: TestMessage) {
      messages.push(message);
      const type = (
        { boot: 'ready', sensor: 'sensor_ready', restart: 'restarted' } as Record<string, string>
      )[message.type];
      if (type)
        queueMicrotask(() =>
          this.onmessage({
            data: {
              type,
              id: message.id,
              state: { virtual_ticks: 0 },
            },
          }),
        );
    },
    terminate() {},
  };
  const engine = new DefaultSceneEngine(
    () => fakeWorker(worker),
    (error) => errors.push(error),
    loadDefault,
  );
  return { engine, messages, errors };
}

test('boot, clear input and card restart supply the still through RGB input', async () => {
  let loads = 0;
  const { engine, messages, errors } = setup(async () => {
    loads++;
    return still;
  });
  try {
    await engine.boot(new ArrayBuffer(1), config, () => {});
    assert.deepEqual(
      new Uint8Array(messages.find((message) => message.type === 'boot')!.scene.rgb),
      still.rgb,
    );
    assert.equal(messages.at(-1)!.type, 'sensor');
    assert.deepEqual(new Uint8Array(messages.at(-1)!.rgb), still.rgb);
    assert.notEqual(messages.at(-1)!.rgb, still.rgb.buffer);
    const custom = { width: 1, height: 1, rgb: new Uint8Array([255, 0, 0]) };
    await engine.setScene(custom);
    await engine.restartWithCardFiles([], () => {});
    assert.deepEqual(
      new Uint8Array(messages.find((message) => message.type === 'restart')!.scene.rgb),
      custom.rgb,
    );
    assert.deepEqual(new Uint8Array(messages.at(-1)!.rgb), custom.rgb);
    await engine.setScene(null);
    assert.deepEqual(new Uint8Array(messages.at(-1)!.rgb), still.rgb);
    await engine.restartWithCardFiles([], () => {});
    assert.deepEqual(new Uint8Array(messages.at(-1)!.rgb), still.rgb);
    assert.equal(loads, 1);
    assert.equal(
      messages.some((message) => message.type === 'sensor_reset'),
      false,
    );
    assert.deepEqual(errors, []);
  } finally {
    engine.dispose();
  }
});

test('an input selected while the default loads takes precedence at boot', async () => {
  let finish!: (value: import('./api').Scene) => void;
  const { engine, messages } = setup(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  try {
    const boot = engine.boot(new ArrayBuffer(1), config, () => {});
    const custom = { width: 1, height: 1, rgb: new Uint8Array([0, 255, 0]) };
    await engine.setScene(custom);
    finish(still);
    await boot;
    assert.deepEqual(new Uint8Array(messages.at(-1)!.rgb), custom.rgb);
  } finally {
    engine.dispose();
  }
});
