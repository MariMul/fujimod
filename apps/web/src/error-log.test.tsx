import { test } from 'vitest';
import assert from 'node:assert/strict';
import { installErrorLog, type ErrorLogRecord } from './error-log';

test('uncaught errors, console errors and emulator faults are logged once each without query or fragment', () => {
  const records: ErrorLogRecord[] = [];
  history.replaceState(null, '', '/dev-tools/?room=secret#token');
  const error = console.error;
  console.error = () => {};
  try {
    installErrorLog((record) => records.push(record));
    const failure = new TypeError("null is not an object (evaluating 'node.nextSibling')");
    dispatchEvent(new ErrorEvent('error', { error: failure, message: failure.message }));
    dispatchEvent(new ErrorEvent('error', { error: failure, message: failure.message }));
    console.error('Hydration mismatch', { node: 'p' });
    dispatchEvent(
      new CustomEvent('fujimod-fault', {
        detail: {
          kind: 'guest-fault',
          message: 'The camera stopped after an execution fault.',
          details: { type: 'GuestExecutionError', error: 'Invalid memory write' },
        },
      }),
    );
  } finally {
    console.error = error;
  }
  assert.deepEqual(
    records.map(({ kind, message }) => [kind, message]),
    [
      ['error', "TypeError: null is not an object (evaluating 'node.nextSibling')"],
      ['console', 'Hydration mismatch {"node":"p"}'],
      [
        'fault',
        'The camera stopped after an execution fault. GuestExecutionError: Invalid memory write',
      ],
    ],
  );
  assert.ok(records.every((record) => record.session === records[0].session));
  assert.equal(records[0].page, `${location.origin}/dev-tools/`);
  assert.ok(records[0].stack?.includes('error-log.test'));
  assert.equal(records[2].fault?.kind, 'guest-fault');
});
