import { flush } from 'solid-js';
import { expect, test } from 'vite-plus/test';
import { Progress } from './camera';
import { ConnectionState, ConsoleState, LineEnding } from './controller';

import { localFixture } from './local-session-fixture';

test('shared local reads and console commands open and close separate sessions, refusing remote claims without USB access', async () => {
  const f = localFixture();
  try {
    await f.start();
    await f.tools.readInspection((read) => read({ kind: 'identity', selector: 'Product' }));
    expect(f.events).toEqual(['claim', 'open', 'identity', 'close', 'release']);
    f.events.length = 0;
    expect(await f.tools.send('hello', LineEnding.Lf)).toBe(true);
    expect(f.events).toEqual(['claim', 'open', 'submit', 'poll', 'close', 'release']);
    f.events.length = 0;
    f.remote(true);
    await expect(f.tools.readInspection(async () => {})).rejects.toThrow('remote controller');
    expect(await f.tools.send('hello', LineEnding.Lf)).toBe(false);
    expect(f.events).toEqual([]);
    flush();
    expect(f.tools.connection()).toBe(ConnectionState.Sharing);
    expect(f.tools.consoleState()).toBe(ConsoleState.Idle);
    expect(f.tools.entries().at(-1)?.message).toContain('remote controller');
    f.remote(false);
    expect(await f.tools.send('hello', LineEnding.Lf)).toBe(true);
  } finally {
    f.tools.dispose();
  }
});

test('pending and unknown console completion retain the local claim until a completed response', async () => {
  const f = localFixture();
  try {
    await f.start();
    f.reply(Progress.Pending);
    await f.tools.send('hello', LineEnding.Lf);
    expect(f.claimed()).toBe(true);
    await expect(f.tools.readInspection(async () => {})).rejects.toThrow('No idle ff80 session');
    f.reply(Progress.Unknown);
    await f.tools.readResponse();
    expect(f.claimed()).toBe(true);
    f.reply(Progress.Complete);
    await f.tools.readResponse();
    expect(f.claimed()).toBe(false);
    expect(f.events).toEqual([
      'claim',
      'open',
      'submit',
      'poll',
      'poll',
      'poll',
      'close',
      'release',
    ]);
  } finally {
    f.tools.dispose();
  }
});

test('a failed read closes its local session; an unusable one abandons and fences ownership', async () => {
  const f = localFixture();
  try {
    await f.start();
    await expect(
      f.tools.readInspection(async () => {
        throw Error('read refused');
      }),
    ).rejects.toThrow('read refused');
    expect(f.events).toEqual(['claim', 'open', 'close', 'release']);
    f.events.length = 0;
    await expect(
      f.tools.readInspection(async () => {
        f.breakSession();
      }),
    ).rejects.toThrow('completion unknown');
    expect(f.events).toEqual(['claim', 'open', 'abandon', 'fence']);
    flush();
    expect(f.tools.connection()).toBe(ConnectionState.Blocked);
  } finally {
    f.tools.dispose();
  }
});

test('disconnect during a shared local open cannot submit a command on the late session', async () => {
  const f = localFixture();
  try {
    await f.start();
    const opened = await f.backend.open(0);
    f.events.length = 0;
    let finish!: () => void;
    f.backend.open = () =>
      new Promise((resolve) => {
        finish = () => resolve(opened);
      });
    const sending = f.tools.send('hello', LineEnding.Lf);
    await f.tools.disconnect();
    finish();
    expect(await sending).toBe(false);
    expect(f.events).not.toContain('submit');
    expect(f.events).toContain('fence');
    flush();
    expect(f.tools.connection()).toBe(ConnectionState.Disconnected);
  } finally {
    f.tools.dispose();
  }
});

test('a broken session cannot overwrite the read failure during cleanup', async () => {
  const f = localFixture();
  try {
    await f.start();
    await expect(
      f.tools.readInspection(async () => {
        f.breakSession();
        throw Error('original read failure');
      }),
    ).rejects.toThrow('original read failure');
    expect(f.events).toEqual(['claim', 'open', 'abandon', 'fence']);
    flush();
    expect(f.tools.inspectionBusy()).toBe(false);
    expect(f.tools.connection()).toBe(ConnectionState.Blocked);
  } finally {
    f.tools.dispose();
  }
});
