import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { scheduleHostTask } from './host-task';

test('host tasks yield a turn and cancellation suppresses queued and delayed work', async () => {
  const calls: string[] = [];
  scheduleHostTask(() => calls.push('canceled'))();
  scheduleHostTask(() => calls.push('delayed'), 1000)();
  const ready = new Promise<void>((resolve) =>
    scheduleHostTask(() => {
      calls.push('ready');
      resolve();
    }),
  );
  assert.deepEqual(calls, []);
  await ready;
  assert.deepEqual(calls, ['ready']);
});
