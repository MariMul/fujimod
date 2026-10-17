import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { WorkerEngine } from './worker-engine.ts';
import { faultFrom } from './fault.ts';
import { cacheKey, CheckpointStore } from './checkpoint-store.js';
import { WorkerRecovery } from './recovery-worker.ts';

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
test('an LCD-only runtime advances without a sensor preview clock', async () => {
  const { engine, worker, sent, errors } = setup();
  await engine.boot(new ArrayBuffer(1), {inputs:'inputs.json',bundle:'runtime/',boot:{sceneInput:false}}, () => {});
  worker.reply({type:'metrics',runtime:{preview:null}});
  worker.postMessage = data => {
    sent.push(data);
    queueMicrotask(() => worker.reply({type:'result',id:data.id,
      state:{virtual_ticks:120100000,display:{width:1,height:1}},frame:new ArrayBuffer(0)}));
  };
  engine.wall -= 1000;
  await engine.pump();
  assert.deepEqual(errors, []);
  assert.ok(Number.isFinite(sent.find(data=>data.action==='advance').options.pause_at_tick));
  engine.dispose();
});
test('a structured fault pauses inputs and retains the LCD; rollback republishes and resumes', async () => {
  const { worker, engine, sent, errors, frames } = setup();
  const events = []; engine.errors(event => events.push(event));
  await engine.boot(new ArrayBuffer(1), { inputs: 'inputs.json', bundle: 'runtime/', boot: { sceneInput: true } }, () => {});
  const pending = engine.press('menu'); await Promise.resolve();
  worker.reply({ type: 'error', id: sent.at(-1).id, fault: faultFrom({ type: 'UnsupportedDevice', error: 'unmodeled operation', pc: '0x123' }) });
  await assert.rejects(pending);
  assert.equal(events.length, 1); assert.equal(errors.length, 1);
  assert.equal(events[0].kind, 'unsupported-device'); assert.deepEqual(events[0].details.pc, '0x123');
  assert.equal(frames.length, 1);
  await assert.rejects(engine.press('menu')); assert.equal(sent.at(-1).action, 'press');
  const restored = engine.rollback(10), request = sent.at(-1);
  assert.equal(request.type, 'rollback'); assert.equal(request.seconds, 10);
  worker.reply({ type: 'result', id: request.id, state: { virtual_ticks: 0, display: { width: 1, height: 1 }, input_positions: { shutter_speed: '1/125', power: 'on' } }, frame: new Uint8Array([4,5,6]).buffer });
  assert.deepEqual(await restored, { shutter_speed: '1/125', power: 'on' });
  assert.equal(frames.length, 2);
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
test('repeated faults can resume the same retained save point', async () => {
  const { worker, engine, sent, errors, frames } = setup();
  await engine.boot(new ArrayBuffer(1), { inputs: 'inputs.json', bundle: 'runtime/', boot: { sceneInput: true } }, () => {});
  const saved = { virtual_ticks: 120000000, display: { width: 1, height: 1 } };
  try {
    for (let attempt = 0; attempt < 3; attempt++) {
      worker.reply({ type: 'error', fault: { ...faultFrom(Error('test fault')), undoSeconds: 10 } });
      await assert.rejects(engine.press('menu'));
      const restored = engine.rollback(10), request = sent.at(-1);
      assert.equal(request.type, 'rollback');
      worker.reply({ type: 'result', id: request.id, state: saved,
        frame: new Uint8Array([4,5,6]).buffer });
      await restored;
      const press = engine.press('menu'); await Promise.resolve();
      worker.reply({ type: 'result', id: sent.at(-1).id, state: saved });
      await press;
    }
    assert.equal(errors.length, 3);
    assert.equal(frames.length, 4);
  } finally { engine.dispose(); }
});
test('rollback availability is read from live history and fails closed if inspection fails', () => {
  const recovery = new WorkerRecovery();
  assert.equal(recovery.canRollback(undefined, 10), false);
  let available = false;
  const py = {
    globals: { set(name, value) { assert.equal(name, 'browser_undo_seconds'); assert.equal(value, 10); } },
    runPython() { return available; },
  };
  assert.equal(recovery.canRollback(py, 10), false);
  available = true;
  assert.equal(recovery.canRollback(py, 10), true);
  py.runPython = () => { throw Error('runtime trapped'); };
  assert.equal(recovery.canRollback(py, 10), false);
});
test('fault events preserve the worker-confirmed undo availability', async () => {
  for (const undoSeconds of [undefined, 10]) {
    const { worker, engine } = setup();
    const faults = []; engine.errors(fault => faults.push(fault));
    await engine.boot(new ArrayBuffer(1), { inputs: 'inputs.json', bundle: 'runtime/', boot: { sceneInput: true } }, () => {});
    worker.reply({ type: 'error', fault: { ...faultFrom(Error('stopped')), undoSeconds } });
    assert.equal(faults[0].undoSeconds, undoSeconds);
    engine.dispose();
  }
});
test('new guest images are presented immediately while unchanged status polls omit pixels', async () => {
  const { engine, worker, sent, frames } = setup();
  await engine.boot(new ArrayBuffer(1), { inputs: 'inputs.json', bundle: 'runtime/', boot: { sceneInput: true } }, () => {});
  worker.reply({ type: 'metrics', runtime: { preview: { frames_per_second: 10 } } });
  let image = 1, tick = 120001000;
  worker.postMessage = data => {
    sent.push(data);
    queueMicrotask(() => worker.reply({ type: 'result', id: data.id,
      state: { virtual_ticks: tick, image_frames: image, display: { width: 1, height: 1 } },
      frame: data.action === 'state' && data.options.pixels ? new Uint8Array([4,5,6]).buffer : new ArrayBuffer(0) }));
  };
  engine.wall -= 1000;
  await engine.pump();
  assert.equal(sent.at(-1).action, 'state'); assert.equal(sent.at(-1).options.pixels, true);
  assert.ok(sent.at(-2).options.pause_at_tick <= 120600000, 'A slice must yield within half a guest image interval');
  assert.equal(frames.length, 2);
  engine.nextFrame = 0;
  await engine.pump();
  assert.equal(sent.at(-1).options.pixels, false); assert.equal(frames.length, 2);
  image++; tick++;
  await engine.pump();
  assert.equal(sent.at(-1).options.pixels, true); assert.equal(frames.length, 3);
  engine.dispose();
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

test('an outstanding browser write prevents overlapping archive exports', async () => {
  let enter, finish;
  const entered = new Promise(resolve => { enter = resolve; });
  const pending = new Promise(resolve => { finish = resolve; });
  const dir = { getFileHandle: async () => ({ createWritable: async () => { enter(); await pending;
    return { write: async () => {}, close: async () => {} }; } }) };
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { storage: { getDirectory: async () => ({ getDirectoryHandle: async () => dir }) } } });
  let exports = 0, destroyed = 0;
  const py = { runPython(source) {
    if (source === 'browser_history.capture(session)') return true;
    exports++;
    return { getBuffer: () => ({ data: new Uint8Array([1,2,3]), release() {} }), destroy() { destroyed++; } };
  } };
  try {
    const recovery = new WorkerRecovery(), saved = recovery.capture(py);
    await entered;
    await recovery.capture(py); await recovery.capture(py);
    assert.equal(exports, 1); assert.equal(destroyed, 1);
    finish(); assert.equal(await saved, true);
    await recovery.capture(py); assert.equal(exports, 1);
  } finally { finish(); if (previous) Object.defineProperty(globalThis, 'navigator', previous); else delete globalThis.navigator; }
});
