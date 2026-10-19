import { testBoot, testConfig, fakeWorker, fakeRuntime, type TestMessage } from './test-fixtures';
import { test, vi } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { CameraPace } from './performance.ts';
import { WorkerEngine } from './worker-engine.ts';

test('speed includes startup and rebasing restored time preserves its wall cost', () => {
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
    assert.equal(pace.sample(132_000_000).speed, 1 / 11);
  } finally { clock.mockRestore(); }
});

test('ready does not discard cold boot time or count restored uptime as execution', async () => {
  const clock = vi.spyOn(performance, 'now');
  Object.assign(globalThis, {document: { baseURI: 'https://example.invalid/' }});
  Object.assign(globalThis, {window: { setTimeout: () => 0 }});
  globalThis.fetch = async () => new Response(JSON.stringify({ inputs: [] }));
  try {
    for (const origin of [0, 120_000_000]) {
      clock.mockReturnValue(0);
      const worker = {
    onmessage: (_event: {data: unknown}) => {},
    onerror: (_event: {message: string; filename?: string; lineno?: number; colno?: number}) => {},
    terminated: false,
        postMessage(message: TestMessage) {
          if (message.type !== 'boot') return;
          clock.mockReturnValue(10000);
          queueMicrotask(() => this.onmessage({data: {type: 'ready',
            state: { virtual_ticks: origin + 6_000_000 }, runtime: { clockOriginTicks: origin }}}));
        }, terminate() {},
      };
      const engine = new WorkerEngine(() => fakeWorker(worker), () => {});
      try {
        await engine.boot(new ArrayBuffer(1), {firmwareId: 'test', inputs: 'inputs.json', bundle: 'runtime/', boot: {...testBoot,sceneInput: false}}, () => {});
        assert.equal(engine['pace'].sample(origin + 6_000_000).speed, 0.05);
      } finally { engine.dispose(); }
    }
  } finally { clock.mockRestore(); }
});
