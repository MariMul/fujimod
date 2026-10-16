import test from 'node:test';
import assert from 'node:assert/strict';
import { WorkerEngine } from './worker-engine.ts';

const state = revision => ({ supported: true, present: true, busy: false, revision, capacity_bytes: 4294967296 });
const config = { bundle: 'runtime/test/', inputs: 'runtime/test/inputs.json', boot: {} };
function setup() {
  globalThis.document = { baseURI: 'https://example.invalid/' };
  globalThis.window = { setTimeout: () => 0 };
  globalThis.fetch = async () => new Response(JSON.stringify({ inputs: [] }));
  const messages = [], errors = [];
  const worker = {
    postMessage(message, transfer) {
      messages.push({ ...message, transfer });
      if (message.type === 'boot') queueMicrotask(() => this.reply({ type: 'ready',
        state: { virtual_ticks: 0 }, frame: new ArrayBuffer(0), card: state(5) }));
    },
    reply(data) { this.onmessage({ data }); },
    terminate() { this.terminated = true; },
  };
  return { worker, messages, errors, engine: new WorkerEngine(() => worker, e => errors.push(e)) };
}

test('preload buffers are copied for transfer; file reads return binary; changes unsubscribe', async () => {
  const { engine, worker, messages, errors } = setup();
  const changes = [], stop = engine.cardChanges(card => changes.push(card));
  const ffboot = new Uint8Array([0, 255, 1]);
  await engine.boot(new ArrayBuffer(3), config, () => {}, [{ path: '/ffboot.bin', bytes: ffboot }]);
  const boot = messages[0];
  assert.equal(boot.cardFiles[0].path, '/ffboot.bin');
  assert.notEqual(boot.cardFiles[0].bytes, ffboot.buffer);
  assert.deepEqual(new Uint8Array(boot.cardFiles[0].bytes), ffboot);
  assert.deepEqual(changes.map(c => c.revision), [5]);
  const listing = engine.cardTree(5), listed = messages.at(-1);
  assert.deepEqual({ type: listed.type, action: listed.action, revision: listed.revision },
    { type: 'card', action: 'tree', revision: 5 });
  const tree = { ...state(5), entries: [{ path: '/DSCF0001.JPG', name: 'DSCF0001.JPG', kind: 'file', bytes: 4 }] };
  worker.reply({ type: 'card_result', id: listed.id, card: state(5), tree });
  assert.deepEqual(await listing, tree);
  assert.equal(changes.length, 1);
  const read = engine.readCardFile('/DSCF0001.JPG', { revision: 5, maxBytes: 64 });
  const requested = messages.at(-1);
  assert.equal(requested.path, '/DSCF0001.JPG');
  assert.equal(requested.maxBytes, 64);
  worker.reply({ type: 'card_result', id: requested.id, card: state(6), bytes: new Uint8Array([255,216,255,217]).buffer });
  assert.deepEqual(await read, new Uint8Array([255,216,255,217]));
  assert.deepEqual(changes.map(c => c.revision), [5,6]);
  stop();
  const status = engine.cardState();
  worker.reply({ type: 'card_result', id: messages.at(-1).id, card: state(7) });
  assert.equal((await status).revision, 7);
  assert.equal(changes.length, 2);
  assert.deepEqual(errors, []);
  engine.dispose();
});

test('a failed file read rejects its request without stopping emulation; dispose rejects pending reads', async () => {
  const { engine, worker, messages, errors } = setup();
  await engine.boot(new ArrayBuffer(3), config, () => {});
  const missing = engine.readCardFile('/MISSING.JPG');
  worker.reply({ type: 'error', id: messages.at(-1).id, error: 'File was not found' });
  await assert.rejects(missing, /not found/);
  assert.deepEqual(errors, []);
  const status = engine.cardState();
  worker.reply({ type: 'card_result', id: messages.at(-1).id, card: state(5) });
  assert.equal((await status).revision, 5);
  const pending = engine.readCardFile('/WAIT.JPG');
  engine.dispose();
  await assert.rejects(pending, /closed/);
  assert.equal(worker.terminated, true);
});

test('card restart transfers copied files, rejects inputs while paused, and resumes notifications', async () => {
  const { engine, worker, messages, errors } = setup();
  await engine.boot(new ArrayBuffer(3), config, () => {});
  const progress = [];
  const payload = new Uint8Array([1,2,3]);
  const restart = engine.restartWithCardFiles([{ path: '/ffboot.bin', bytes: payload }], (...value) => progress.push(value));
  await Promise.resolve();
  const request = messages.at(-1);
  assert.equal(request.type, 'restart');
  assert.deepEqual(new Uint8Array(request.cardFiles[0].bytes), payload);
  assert.notEqual(request.cardFiles[0].bytes, payload.buffer);
  assert.ok(request.transfer.includes(request.cardFiles[0].bytes));
  await assert.rejects(engine.cardTree(), /restarting/);
  await assert.rejects(engine.press('menu_ok'));
  assert.equal(messages.at(-1), request);
  worker.reply({ type: 'progress', message: 'Restarting', percent: 20 });
  worker.reply({ type: 'restarted', id: request.id, state: { virtual_ticks: 1000 }, frame: new ArrayBuffer(0), card: state(6) });
  await restart;
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(progress, [['Restarting',20]]);
  const press = engine.press('menu_ok');
  await new Promise(resolve => setImmediate(resolve));
  const input = messages.findLast(m => m.type === 'request');
  assert.equal(input.action, 'press');
  worker.reply({ type: 'result', id: input.id, state: { virtual_ticks: 1000 }, card: state(6) });
  await press;
  assert.deepEqual(errors, []);
  engine.dispose();
});
