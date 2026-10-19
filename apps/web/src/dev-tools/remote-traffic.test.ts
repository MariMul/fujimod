import { expect, test } from 'vite-plus/test';
import { observeRequest } from '../../../../crates/usb-proxy/web/share-traffic.ts';
import { RemoteTrafficLog } from './remote-traffic';
import { Direction, Layer, Level } from './log';
import type { Completion, Operation } from './usb-sharing';
function words(...values: number[]) {
  const data = new Uint8Array(values.length * 4),
    view = new DataView(data.buffer);
  values.forEach((value, index) => view.setUint32(index * 4, value, true));
  return data;
}
function join(...chunks: Uint8Array[]) {
  const result = new Uint8Array(chunks.reduce((n, chunk) => n + chunk.length, 0));
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }
  return result;
}
function fixture() {
  let now = 0,
    id = 0;
  const log = new RemoteTrafficLog(() => now);
  log.begin({
    id: 'camera',
    vendor_id: 0x4cb,
    product_id: 0xff80,
    manufacturer: null,
    product: 'Synthetic',
    serial: null,
    interfaces: [
      {
        configuration: 1,
        interface: 2,
        alternate: 3,
        endpoints: [
          { packet_size: 64, address: 1, kind: 'bulk' },
          { packet_size: 64, address: 0x82, kind: 'bulk' },
          { packet_size: 64, address: 0x83, kind: 'interrupt' },
        ],
      },
    ],
  });
  const send = (operation: Operation, completion: Completion, controller = 'native') =>
    observeRequest(
      controller,
      { id: ++id, device: 'camera', operation, timeout_ms: 5000 },
      async () => completion,
      (event) => log.record(event),
    );
  const control = (kind: number, size: number, controller = 'native') => {
    const data = new Uint8Array(12);
    data[0] = kind;
    new DataView(data.buffer).setUint32(6, size);
    return send(
      {
        op: 'control_out',
        setup: { request_type: 'vendor', recipient: 'other', request: 0, value: 0, index: 0 },
        data: data,
      },
      { transferred: 12 },
      controller,
    );
  };
  const out = (data: Uint8Array, controller = 'native') =>
    send({ op: 'transfer_out', endpoint: 1, data: data }, { transferred: data.length }, controller);
  const input = (data: Uint8Array, length = 64, controller = 'native', endpoint = 0x82) =>
    send(
      { op: 'transfer_in', endpoint, length },
      { data: data, transferred: data.length },
      controller,
    );
  return {
    log,
    send,
    control,
    out,
    input,
    advance: (ms: number) => {
      now += ms;
    },
  };
}
test('the real request observer decodes fragmented FF80 commands and console text in both directions', async () => {
  const f = fixture();
  await f.control(0xc0, 8);
  await f.input(new Uint8Array(8));
  const command = new TextEncoder().encode('hello\n');
  const submit = join(words(0x200001, 0, 0, 0, 6, command.length), command, words(0));
  await f.control(0xd0, submit.length);
  await f.out(submit.slice(0, 3));
  await f.out(submit.slice(3, 19));
  await f.out(submit.slice(19));
  await f.control(0xd1, 32);
  await f.input(words(0x200001, 0, 0, 0, 6));
  const poll = words(0x200001, 0, 0, 0, 7, 4096, 0, 0);
  await f.control(0xd0, poll.length);
  await f.out(poll);
  const output = new TextEncoder().encode('Hello, world!\n');
  const reply = join(words(0x200001, 0, 0, 0, 7, 0, 0, output.length), output);
  await f.control(0xd1, 4096);
  await f.input(reply.slice(0, 16), 16);
  await f.input(reply.slice(16));
  const entries = f.log.drain(true);
  expect(
    entries.filter((e) => e.layer === Layer.Ff80).map((e) => [e.direction, e.message]),
  ).toEqual([
    [Direction.Out, 'Open session'],
    [Direction.In, 'Open session'],
    [Direction.Out, 'Console submit'],
    [Direction.In, 'Console submit'],
    [Direction.Out, 'Console poll'],
    [Direction.In, 'Console poll'],
  ]);
  expect(
    entries.filter((e) => e.layer === Layer.Console).map((e) => [e.direction, e.message]),
  ).toEqual([
    [Direction.Out, 'hello'],
    [Direction.In, 'Hello, world!'],
  ]);
  expect(entries.every((e) => e.detail?.includes('native'))).toBe(true);
});
test('framed device status errors retain their FF80 command and raw USB success is separate', async () => {
  const f = fixture(),
    request = words(0x200001, 0, 0, 0, 2, 0, 0, 0x1234, 4);
  await f.control(0xd0, request.length);
  await f.out(request);
  await f.control(0xd1, 40);
  await f.input(words(0x200001, 1, 0xffff, 0, 2));
  const entries = f.log.drain(true),
    failure = entries.find((e) => e.message === 'Read memory · failed')!;
  expect(failure.level).toBe(Level.Error);
  expect(failure.detail).toContain('Device status 0x1/0xffff');
  expect(entries.some((e) => e.layer === Layer.Connection && e.message.includes('ok'))).toBe(true);
});
test('decode state stays separate for interleaved controllers', async () => {
  const f = fixture(),
    a = words(0x200001, 0, 0, 0, 2, 0, 0, 0, 1),
    b = words(0x150000, 0, 0, 0, 4);
  await f.control(0xd0, a.length, 'A');
  await f.out(a, 'A');
  await f.control(0xd0, b.length, 'B');
  await f.out(b, 'B');
  await f.control(0xd1, 64, 'B');
  await f.input(words(0x150000, 0, 0, 0), 64, 'B');
  await f.control(0xd1, 64, 'A');
  await f.input(a, 64, 'A');
  const replies = f.log
    .drain(true)
    .filter((e) => e.layer === Layer.Ff80 && e.direction === Direction.In);
  expect(replies.map((e) => [e.message, e.detail?.includes('Controller B')])).toEqual([
    ['Device info', true],
    ['Read memory', false],
  ]);
  expect(replies[0].correlation).not.toBe(replies[1].correlation);
});
test('unframed transfers, standard controls, interrupt endpoints and mismatched echoes stay raw', async () => {
  const f = fixture(),
    command = words(0x200001, 0, 0, 0, 7, 4, 0, 0);
  await f.out(command);
  await f.control(0xd0, command.length);
  await f.out(command);
  await f.control(0xd1, 64);
  await f.input(command, 64, 'native', 0x83);
  await f.input(words(0x200001, 0, 0, 0, 6));
  const fake = new Uint8Array(12);
  fake[0] = 0xd0;
  await f.send(
    {
      op: 'control_out',
      setup: { request_type: 'standard', recipient: 'device', request: 6, value: 0, index: 0 },
      data: fake,
    },
    { transferred: 12 },
  );
  await f.out(command);
  const entries = f.log.drain(true);
  expect(
    entries.filter((e) => e.layer === Layer.Ff80).map((e) => [e.direction, e.message]),
  ).toEqual([[Direction.Out, 'Console poll']]);
  expect(entries.some((e) => e.layer === Layer.Connection && e.message.includes('ep 0x83'))).toBe(
    true,
  );
});
test('ten thousand data chunks and their envelopes collapse into two accurate summaries', async () => {
  const f = fixture(),
    data = new Uint8Array(64);
  const envelope = new Uint8Array(12);
  envelope[0] = 0xe1;
  new DataView(envelope.buffer).setUint32(6, 64);
  for (let i = 0; i < 10000; i++) {
    for (const [operation, completion] of [
      [
        {
          op: 'control_out',
          setup: { request_type: 'vendor', recipient: 'other', request: 0, value: 0, index: 0 },
          data: envelope,
        },
        { transferred: 12 },
      ],
      [
        { op: 'transfer_in', endpoint: 0x82, length: 64 },
        { data, transferred: 64 },
      ],
    ] as const) {
      const event = {
        controller: 'bulk-reader',
        device: 'camera',
        timeoutMs: 5000,
        id: i,
        operation,
      };
      f.log.record({ ...event, phase: 'submitted' });
      f.log.record({
        ...event,
        phase: 'completed',
        elapsedMs: 1,
        completion: { ...completion, deviceCount: 0 },
      });
    }
  }
  expect(f.log.drain()).toHaveLength(0);
  f.advance(250);
  const entries = f.log.drain();
  expect(entries).toHaveLength(2);
  expect(entries.find((e) => e.message.includes('transfer_in'))!.message).toContain(
    '10000 submitted, 10000 completed · requested 640000 B, transferred 640000 B · ok',
  );
  expect(entries.every((e) => (e.data?.length ?? 0) <= 256)).toBe(true);
});
test('pending requests, short writes and USB failures report actual sizes without inventing a reply', async () => {
  const f = fixture(),
    event = {
      controller: 'slow',
      id: 7,
      timeoutMs: 1000,
      operation: { op: 'transfer_in', endpoint: 0x82, length: 128 },
    };
  f.log.record({ ...event, phase: 'submitted' });
  f.advance(300);
  expect(f.log.drain()[0].message).toContain(
    '1 submitted, 0 completed · requested 128 B, transferred 0 B · pending',
  );
  f.log.record({
    ...event,
    phase: 'completed',
    elapsedMs: 1000,
    completion: {
      transferred: 12,
      error: 'USB deadline expired; completion unknown',
      deviceCount: 0,
    },
  });
  const failure = f.log.drain()[0];
  expect(failure.level).toBe(Level.Error);
  expect(failure.message).toContain('transferred 12 B');
  expect(failure.message).toContain('completion unknown');
  const command = words(0x200001, 0, 0, 0, 6, 0, 0);
  await f.control(0xd0, command.length);
  await f.send({ op: 'transfer_out', endpoint: 1, data: command }, { transferred: 3 });
  expect(f.log.end().some((e) => e.layer === Layer.Ff80)).toBe(false);
});
test('large command bodies and decoded-event floods stay bounded and report truncation and loss', async () => {
  const f = fixture(),
    payload = new Uint8Array(100000).fill(97);
  const command = join(words(0x200001, 0, 0, 0, 6, payload.length), payload, words(0));
  await f.control(0xd0, command.length);
  await f.out(command);
  const console = f.log.drain().find((e) => e.layer === Layer.Console)!;
  expect(console.message.length).toBeLessThanOrEqual(4096);
  expect(console.detail).toContain('of 100000 B');
  for (let i = 0; i < 1000; i++) {
    await f.control(0xd0, 16);
    await f.out(words(0x150000, 0, 0, 0));
  }
  const entries = f.log.drain(true);
  expect(entries.length).toBeLessThan(200);
  expect(entries.reduce((n, e) => n + (e.data?.length ?? 0), 0)).toBeLessThan(1024 * 1024);
  expect(entries.some((e) => e.level === Level.Warning && e.message.includes('discarded'))).toBe(
    true,
  );
});
test('ending a share flushes summaries and clears partial decoder state before a new owner', async () => {
  const f = fixture(),
    request = words(0x200001, 0, 0, 0, 2, 0, 0, 0, 1);
  await f.control(0xd0, request.length);
  await f.out(request.slice(0, 16));
  expect(f.log.end('native').some((e) => e.message.includes('transfer_out'))).toBe(true);
  await f.out(request.slice(16));
  expect(f.log.drain(true).some((e) => e.layer === Layer.Ff80)).toBe(false);
});

test('clearing visible traffic retains the pending command decoder but removes queued old rows', async () => {
  const f = fixture(),
    command = words(0x200001, 0, 0, 0, 2, 0, 0, 0, 1);
  await f.control(0xd0, command.length);
  await f.out(command);
  f.log.clear();
  expect(f.log.drain(true)).toHaveLength(0);
  await f.control(0xd1, 64);
  await f.input(command);
  const decoded = f.log.drain(true).filter((e) => e.layer === Layer.Ff80);
  expect(decoded.map((e) => [e.direction, e.message])).toEqual([[Direction.In, 'Read memory']]);
});
test('large binary traffic retains full byte counts while observations keep bounded prefixes', async () => {
  const f = fixture(),
    data = new Uint8Array(8 * 1024 * 1024).fill(255);
  await f.send({ op: 'transfer_out', endpoint: 1, data }, { transferred: data.length });
  const entries = f.log.drain(true);
  expect(entries).toHaveLength(1);
  expect(entries[0].message).toContain('requested 8388608 B, transferred 8388608 B');
  expect(entries[0].data?.length).toBe(256);
});
