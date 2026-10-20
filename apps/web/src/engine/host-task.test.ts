import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { HostTasks } from './host-task';

test('host tasks yield a turn and cancellation suppresses queued and delayed work', async () => {
  const calls: string[] = [];
  const tasks = new HostTasks();
  tasks.schedule(() => calls.push('canceled'))();
  tasks.schedule(() => calls.push('delayed'), 1000)();
  const ready = new Promise<void>((resolve) =>
    tasks.schedule(() => {
      calls.push('ready');
      resolve();
    }),
  );
  assert.deepEqual(calls, []);
  await ready;
  assert.deepEqual(calls, ['ready']);
  await new Promise<void>((resolve) => tasks.schedule(resolve));
  assert.deepEqual(calls, ['ready']);
});
