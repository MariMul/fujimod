import { testBoot, testConfig, fakeWorker, fakeRuntime, type TestMessage } from './test-fixtures';
import { test, vi } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { CameraPace } from './performance.ts';
import { WorkerEngine } from './worker-engine.ts';

test('cumulative speed includes startup and excludes restored guest uptime', () => {
  const clock = vi.spyOn(performance, 'now');
  try {
    clock.mockReturnValue(1000);
    const pace = new CameraPace();
    pace.reset(0);
    clock.mockReturnValue(11000);
    assert.equal(pace.sample(6_000_000).speed, 0.05);
    pace.rebase(120_000_000);
    assert.equal(pace.sample(120_000_000).speed, 0);
    clock.mockReturnValue(12000);
    assert.equal(pace.sample(132_000_000).wallSeconds, 11);
    assert.equal(pace.sample(132_000_000).averageSpeed, 1 / 11);
  } finally {
    clock.mockRestore();
  }
});

test('ready does not discard cold boot time or count restored uptime as execution', async () => {
  const clock = vi.spyOn(performance, 'now');
  Object.assign(globalThis, { document: { baseURI: 'https://example.invalid/' } });
  Object.assign(globalThis, { window: { setTimeout: () => 0 } });
  globalThis.fetch = async () => new Response(JSON.stringify({ inputs: [] }));
  try {
    for (const origin of [0, 120_000_000]) {
      clock.mockReturnValue(0);
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
          if (message.type !== 'boot') return;
          clock.mockReturnValue(10000);
          queueMicrotask(() =>
            this.onmessage({
              data: {
                type: 'ready',
                state: { virtual_ticks: origin + 6_000_000 },
                runtime: { clockOriginTicks: origin },
              },
            }),
          );
        },
        terminate() {},
      };
      const engine = new WorkerEngine(
        () => fakeWorker(worker),
        () => {},
      );
      try {
        await engine.boot(
          new ArrayBuffer(1),
          {
            firmwareId: 'test',
            inputs: 'inputs.json',
            bundle: 'runtime/',
            boot: { ...testBoot, sceneInput: false },
          },
          () => {},
        );
        assert.equal(engine['pace'].sample(origin + 6_000_000).averageSpeed, 0.05);
      } finally {
        engine.dispose();
      }
    }
  } finally {
    clock.mockRestore();
  }
});

test('live speed measures recent execution independently of startup and old slow intervals', () => {
  const clock = vi.spyOn(performance, 'now');
  try {
    clock.mockReturnValue(0);
    const pace = new CameraPace();
    pace.reset(0);
    clock.mockReturnValue(60000);
    pace.rebase(120_000_000);
    assert.equal(pace.sample(132_000_000).speed, 0);
    clock.mockReturnValue(65000);
    assert.equal(pace.sample(144_000_000).speed, 0.2);
    clock.mockReturnValue(70000);
    const live = pace.sample(192_000_000);
    assert.equal(live.speed, 0.8);
    assert.equal(live.averageSpeed, 6 / 70);
    assert.equal(live.virtualSeconds, 6);
    assert.equal(live.wallSeconds, 70);
    clock.mockReturnValue(75000);
    assert.equal(pace.sample(192_000_000).speed, 0);
    clock.mockReturnValue(76000);
    assert.equal(pace.sample(132_000_000).speed, 0);
  } finally {
    clock.mockRestore();
  }
});
