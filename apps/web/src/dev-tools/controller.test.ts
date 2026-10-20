import { flush } from 'solid-js';
import { expect, test } from 'vite-plus/test';
import {
  CameraTarget,
  Progress,
  type CameraBackend,
  type CameraLink,
  type ConsoleOutput,
} from './camera';
import { ConnectionState, ConsoleState, LineEnding, createDevTools } from './controller';
import { Direction, Layer, Level, LogBuffer, exportLog, type NewEntry } from './log';
function fixture() {
  let closed = false;
  const commands: Uint8Array[] = [];
  const output: ConsoleOutput[] = [
    { progress: Progress.Complete, status: 0, output: new TextEncoder().encode('hello\n') },
  ];
  const link: CameraLink = {
    submit: async (data) => {
      commands.push(data);
    },
    poll: async () => output.shift()!,
    usable: () => !closed,
    close: async () => {
      closed = true;
    },
    abandon: () => {
      closed = true;
    },
  };
  const pending: NewEntry[] = [];
  const backend: CameraBackend = {
    initialize: async () => {},
    pick: async () => ({ name: 'Body B', interfaces: [{ id: 7, label: 'Interface 7' }] }),
    open: async (id) => {
      expect(id).toBe(7);
      return link;
    },
    drain: () => pending.splice(0),
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  return { backend, link, commands, output, pending, closed: () => closed };
}
test('one-interface connect and send work before a UI batch commits; output shares the log', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  try {
    await tools.start();
    await tools.connect();
    expect(await tools.send('help', LineEnding.Lf)).toBe(true);
    flush();
    expect(tools.connection()).toBe(ConnectionState.Connected);
    expect([...f.commands[0]]).toEqual([...new TextEncoder().encode('help\n')]);
    expect(
      tools
        .entries()
        .filter((e) => e.layer === Layer.Console)
        .map((e) => [e.direction, e.message]),
    ).toEqual([
      [Direction.Out, 'help'],
      [Direction.In, 'hello\n'],
    ]);
    expect(tools.consoleState()).toBe(ConsoleState.Idle);
  } finally {
    tools.dispose();
  }
});
test('busy, pending and unknown completion never permit another command or imply retries', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  let submit!: () => void;
  f.link.submit = (data) => {
    f.commands.push(data);
    return new Promise((resolve) => {
      submit = resolve;
    });
  };
  f.link.poll = async () => {
    tools.stopPolling();
    return { progress: Progress.Pending, status: 0, output: new Uint8Array() };
  };
  try {
    await tools.start();
    await tools.connect();
    const sent = tools.send('work', LineEnding.None);
    expect(await tools.send('duplicate', LineEnding.Lf)).toBe(false);
    submit();
    await sent;
    flush();
    expect(tools.consoleState()).toBe(ConsoleState.Pending);
    expect(await tools.send('overwrite', LineEnding.Lf)).toBe(false);
    f.link.poll = async () => ({
      progress: Progress.Unknown,
      unknown: 17,
      status: 0,
      output: new Uint8Array(),
    });
    await tools.readResponse();
    flush();
    expect(tools.consoleState()).toBe(ConsoleState.Unknown);
    expect(await tools.send('again', LineEnding.Lf)).toBe(false);
    expect(f.commands).toHaveLength(1);
    f.link.poll = async () => ({
      progress: Progress.Complete,
      status: 0,
      output: new Uint8Array(),
    });
    await tools.readResponse();
    flush();
    expect(tools.consoleState()).toBe(ConsoleState.Idle);
    expect(f.commands).toHaveLength(1);
  } finally {
    tools.dispose();
  }
});
test('disconnect during an operation releases the host link without a camera close/reset', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  let finish!: () => void;
  f.link.submit = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  try {
    await tools.start();
    await tools.connect();
    const sending = tools.send('help', LineEnding.CrLf);
    await tools.disconnect();
    finish();
    await sending;
    flush();
    expect(f.closed()).toBe(true);
    expect(tools.connection()).toBe(ConnectionState.Disconnected);
    expect(tools.consoleState()).toBe(ConsoleState.Idle);
  } finally {
    tools.dispose();
  }
});
test('a failed transfer fences command entry and reports the error in the same log', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  f.link.usable = () => false;
  f.link.submit = async () => {
    throw Error('USB timeout');
  };
  try {
    await tools.start();
    await tools.connect();
    await tools.send('help', LineEnding.Lf);
    flush();
    expect(tools.connection()).toBe(ConnectionState.Blocked);
    expect(tools.entries().at(-1)).toMatchObject({ level: Level.Error, message: 'USB timeout' });
    expect(await tools.send('retry', LineEnding.Lf)).toBe(false);
  } finally {
    tools.dispose();
  }
});
test('log capture bounds raw bytes and entries; export preserves the actual data', () => {
  const buffer = new LogBuffer(2, 128);
  const entry = {
    layer: Layer.Usb,
    level: Level.Trace,
    direction: Direction.Out,
    message: 'bulk',
    data: new Uint8Array([0, 255]),
  };
  buffer.append([entry, entry, entry]);
  expect(buffer.discarded).toBe(1);
  expect(buffer.entries).toHaveLength(2);
  expect(JSON.parse(exportLog(buffer.entries))[0].data).toBe('00 ff');
  buffer.append([{ ...entry, data: new Uint8Array(129) }]);
  expect(buffer.discarded).toBe(2);
  expect(buffer.entries).toHaveLength(2);
});

test('invalid console text is rejected locally before any submission', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  try {
    await tools.start();
    await tools.connect();
    for (const text of ['é', 'one\ntwo', 'nul\0', 'x'.repeat(4096)]) {
      expect(await tools.send(text, LineEnding.CrLf)).toBe(false);
    }
    expect(f.commands).toHaveLength(0);
  } finally {
    tools.dispose();
  }
});
test('simulator can connect without physical WebUSB; physical USB remains unavailable', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, false);
  const picked: unknown[] = [];
  f.backend.pick = async (target) => {
    picked.push(target);
    return { name: 'Emulated firmware', interfaces: [{ id: 7, label: 'USB' }] };
  };
  try {
    await tools.start();
    await tools.connect();
    expect(picked).toHaveLength(0);
    await tools.connect(CameraTarget.Emulator);
    flush();
    expect(tools.connection()).toBe(ConnectionState.Connected);
    expect(await tools.send('help', LineEnding.Lf)).toBe(true);
    await tools.disconnect();
    flush();
    expect(tools.connection()).toBe(ConnectionState.Unsupported);
  } finally {
    tools.dispose();
  }
});
test('a canceled boot failure cannot replace a newer connection', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  let reject!: (error: Error) => void;
  f.backend.pick = () =>
    new Promise((_resolve, rejected) => {
      reject = rejected;
    });
  try {
    await tools.start();
    const old = tools.connect(CameraTarget.Emulator);
    await tools.disconnect();
    f.backend.pick = async () => ({ name: 'New camera', interfaces: [{ id: 7, label: 'USB' }] });
    const fresh = tools.connect(CameraTarget.Emulator);
    reject(Error('Old emulator was canceled'));
    await Promise.all([old, fresh]);
    flush();
    expect(tools.connection()).toBe(ConnectionState.Connected);
    expect(tools.entries().some((e) => e.message === 'Old emulator was canceled')).toBe(false);
  } finally {
    tools.dispose();
  }
});

test('a shared camera controller does not require local WebUSB support', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, false);
  try {
    await tools.start();
    await tools.connect(CameraTarget.Remote);
    flush();
    expect(tools.connection()).toBe(ConnectionState.Connected);
  } finally {
    tools.dispose();
  }
});

test('workflow session access requires an idle connected ff80 session', async () => {
  for (const target of [CameraTarget.Usb, CameraTarget.Emulator, CameraTarget.Remote]) {
    const f = fixture();
    const session = {
      armResetGuard: async () => ({ guardWasSet: false, debugFlagBefore: 0, progress: [] }),
      enterDebugMode: async () => ({ alreadyPersisted: false, progress: [] }),
      leaveDebugMode: async () => ({ alreadyPersisted: false, progress: [] }),
    };
    f.link.debugSession = session;
    const tools = createDevTools(f.backend, true);
    try {
      await tools.start();
      flush();
      expect(tools.debugSession()).toBeUndefined();
      await tools.connect(target);
      flush();
      expect(tools.debugSession()).toBe(session);
      await tools.disconnect();
      flush();
      expect(tools.debugSession()).toBeUndefined();
    } finally {
      tools.dispose();
    }
  }
});
