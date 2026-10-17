import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runInNewContext } from 'node:vm';
import { browserDiagnostics } from '../scripts/browser-diagnostics.mjs';

test('development diagnostics capture async failures, console panics and renderer state without URL credentials', async () => {
  const sent = [],
    events = {},
    timers = [];
  const browserConsole = { error() {}, warn() {} };
  const source = await readFile(
    new URL('../scripts/browser-diagnostics.js', import.meta.url),
    'utf8',
  );
  runInNewContext(source, {
    crypto: { randomUUID: () => 'test-session' },
    console: browserConsole,
    navigator: { userAgent: 'test Safari', gpu: {} },
    location: { pathname: '/' },
    performance: { now: () => 5000, getEntriesByType: () => [] },
    document: {
      visibilityState: 'visible',
      addEventListener() {},
      querySelector: () => ({
        dataset: { renderer: 'bevy', simulation: '{"ready":false}', renderFrames: '2' },
      }),
    },
    window: {
      addEventListener: (type, callback) => {
        events[type] = callback;
      },
    },
    fetch: async (_url, request) => sent.push(JSON.parse(request.body)),
    setTimeout: (callback) => timers.push(callback),
  });
  browserConsole.error('Bevy GPU panic');
  events.unhandledrejection({
    reason: 'GPU failure https://example.com/file?token=secret#fragment',
  });
  events.error({ message: 'shader validation failed' });
  timers[0]();
  assert.deepEqual(
    sent.map((entry) => entry.kind),
    ['startup', 'console.error', 'unhandledrejection', 'error', 'checkpoint'],
  );
  assert.equal(sent[2].detail, 'GPU failure https://example.com/file');
  assert.equal(sent[4].simulation, '{"ready":false}');
  assert.equal(sent[4].webgpu, true);
  events['camera-boot-memory']({ detail: { stage: 'board-created', wasmMemoryBytes: 600000000, peakWasmMemoryBytes: 700000000 } });
  assert.equal(sent[5].kind, 'emulator-memory');
  assert.equal(sent[5].emulatorBootStage, 'board-created');
  assert.equal(sent[5].emulatorMemoryBytes, 600000000);
  assert.equal(sent[5].emulatorPeakMemoryBytes, 700000000);
});

test('diagnostic endpoint saves valid reports and rejects oversized, malformed and cross-origin requests', async () => {
  const root = await mkdtemp(join(tmpdir(), 'fujimod-browser-'));
  const logs = [];
  let middleware;
  const plugin = browserDiagnostics();
  assert.equal(plugin.apply, 'serve');
  plugin.configureServer({
    config: { root, logger: { info: (line) => logs.push(line) } },
    middlewares: {
      use: (handler) => {
        middleware = handler;
      },
    },
  });
  const server = createServer((req, res) =>
    middleware(req, res, () => {
      res.statusCode = 404;
      res.end();
    }),
  );
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}/__browser-diagnostics`;
  const post = (body, headers = {}) => fetch(url, { method: 'POST', body, headers });
  try {
    assert.equal(
      (await post(JSON.stringify({ session: 'phone', kind: 'console.error', detail: 'GPU panic' })))
        .status,
      204,
    );
    assert.equal((await post('not json')).status, 400);
    assert.equal((await post('x'.repeat(32769))).status, 413);
    assert.equal((await post('{}', { Origin: 'https://other.example' })).status, 403);
    assert.equal((await post('{}', { Origin: 'invalid' })).status, 403);
    const report = JSON.parse(
      await readFile(join(root, 'build/web-service/browser-diagnostics.jsonl'), 'utf8'),
    );
    assert.equal(report.detail, 'GPU panic');
    assert.equal(logs.length, 1);
  } finally {
    await new Promise((resolve) => server.close(resolve));
    await rm(root, { recursive: true });
  }
});
