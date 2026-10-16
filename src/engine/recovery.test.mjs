import test from 'node:test';
import assert from 'node:assert/strict';
import { WorkerEngine } from './worker-engine.ts';
import { faultFrom } from './fault.ts';
import { cacheKey, CheckpointStore } from './checkpoint-store.js';

function setup() {
  globalThis.document = { baseURI: 'https://example.invalid/' };
  globalThis.window = { setTimeout: () => 0 };
  globalThis.fetch = async () => new Response(JSON.stringify({ inputs: [] }));
  const sent = [], errors = [], frames = [];
  const worker = { postMessage(data) {
    sent.push(data);
    if (data.type === 'boot') queueMicrotask(() => this.reply({ type: 'ready', state: { virtual_ticks: 120000000, display: { width: 1, height: 1 } }, frame: new Uint8Array([1,2,3]).buffer }));
  }, reply(data) { this.onmessage({ data }); }, terminate() {} };
  const engine = new WorkerEngine(() => worker, error => errors.push(error));
  engine.frames(frame => frames.push(frame));
  return { worker, engine, sent, errors, frames };
}
test('a structured fault pauses inputs and retains the LCD; rollback republishes and resumes', async () => {
  const { worker, engine, sent, errors, frames } = setup();
  const events = []; engine.errors(event => events.push(event));
  await engine.boot(new ArrayBuffer(1), { inputs: 'inputs.json', bundle: 'runtime/' }, () => {});
  const pending = engine.press('menu'); await Promise.resolve();
  worker.reply({ type: 'error', id: sent.at(-1).id, fault: faultFrom({ type: 'UnsupportedDevice', error: 'unmodeled operation', pc: '0x123' }) });
  await assert.rejects(pending);
  assert.equal(events.length, 1); assert.equal(errors.length, 1);
  assert.equal(events[0].kind, 'unsupported-device'); assert.deepEqual(events[0].details.pc, '0x123');
  assert.equal(frames.length, 1);
  await assert.rejects(engine.press('menu')); assert.equal(sent.at(-1).action, 'press');
  const restored = engine.rollback(10), request = sent.at(-1);
  assert.equal(request.type, 'rollback'); assert.equal(request.seconds, 10);
  worker.reply({ type: 'result', id: request.id, state: { virtual_ticks: 0, display: { width: 1, height: 1 } }, frame: new Uint8Array([4,5,6]).buffer });
  await restored; assert.equal(frames.length, 2);
  const press = engine.press('menu'); await Promise.resolve();
  worker.reply({ type: 'result', id: sent.at(-1).id, state: { virtual_ticks: 1 } });
  await press; engine.dispose();
});
test('fault normalization distinguishes guest faults, unsupported models and Wasm traps', () => {
  assert.equal(faultFrom(new WebAssembly.RuntimeError('unreachable')).kind, 'wasm-trap');
  assert.equal(faultFrom({ cpu: 0, message: 'write failed' }).kind, 'guest-fault');
  assert.equal(faultFrom({ error_type: 'UnsupportedDevice', error: 'write failed' }).kind, 'unsupported-device');
  assert.equal(faultFrom(new TypeError('broken callback')).kind, 'exception');
});
test('cache identity changes independently with DAT, runtime and storage profile', async () => {
  const keys = await Promise.all([['a','b','c'],['A','b','c'],['a','B','c'],['a','b','C']].map(args => cacheKey(...args)));
  assert.equal(new Set(keys).size, 4);
});
test('OPFS rejects corrupt bytes and unavailable persistence degrades to a cold miss', async () => {
  let blob;
  const dir = { getFileHandle: async () => ({ getFile: async () => blob,
    createWritable: async () => ({ write: async value => { blob = value; }, close: async () => {}, abort: async () => {} }) }) };
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { storage: { getDirectory: async () => ({ getDirectoryHandle: async () => dir }) } } });
  try {
    const store = new CheckpointStore(), bytes = new Uint8Array([1,2,3]);
    assert.equal(await store.write('key', bytes), true); assert.equal(store.backend, 'opfs');
    assert.deepEqual(new Uint8Array(await store.read('key')), bytes);
    blob = new Blob([new Uint8Array([0,1,2])]); assert.equal(await store.read('key'), undefined);
    globalThis.navigator.storage.getDirectory = async () => { throw Error('no permission'); };
    assert.equal(await store.write('key', bytes), false); assert.equal(await store.read('key'), undefined);
  } finally { if (previous) Object.defineProperty(globalThis, 'navigator', previous); else delete globalThis.navigator; }
});
