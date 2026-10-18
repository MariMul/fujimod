import test from 'node:test';
import assert from 'node:assert/strict';
import { observeRequest } from './share-traffic.js';
const request = () => ({
  id: 23, device: 'camera', timeout_ms: 5000,
  operation: { op: 'control_out', setup: { request: 0 }, data: 'AQID' },
});
test('each request and completion identify the controller, operation, bytes and result', async () => {
  const events = [];
  let finish;
  const pending = observeRequest('controller-2', request(), () => new Promise(resolve => { finish = resolve; }), event => events.push(event));
  assert.equal(events.length, 1);
  assert.equal(events[0].phase, 'submitted');
  assert.equal(events[0].controller, 'controller-2');
  assert.equal(events[0].id, 23);
  assert.equal(events[0].operation.data, 'AQID');
  assert.equal(events[0].timeoutMs, 5000);
  finish({ data: 'BAU=', transferred: 2, error: null });
  assert.equal((await pending).transferred, 2);
  assert.equal(events.length, 2);
  assert.equal(events[1].phase, 'completed');
  assert.deepEqual(events[1].completion, { data: 'BAU=', transferred: 2, error: null, deviceCount: 0 });
  assert.ok(events[1].elapsedMs >= 0);
});
test('thrown ownership or execution errors still have a completion', async () => {
  const events = [];
  const completion = await observeRequest('other', request(), async () => { throw Error('Shared camera is in use'); }, event => events.push(event));
  assert.equal(completion.error, 'Shared camera is in use');
  assert.equal(events[1].completion.error, completion.error);
});
test('observer mutations and exceptions cannot modify or prevent the operation or reply', async () => {
  const frame = request();
  let ran = 0, observed = 0;
  const completion = await observeRequest('controller', frame, async () => {
    ran++;
    assert.equal(frame.operation.op, 'control_out');
    assert.equal(frame.operation.setup.request, 0);
    return { data: 'AQID', transferred: 3 };
  }, event => {
    observed++;
    if (event.phase === 'submitted') event.operation.setup.request = 99;
    else event.completion.data = 'changed';
  });
  assert.equal(ran, 1);
  assert.equal(observed, 2);
  assert.equal(completion.data, 'AQID');
  assert.equal(completion.transferred, 3);
});
