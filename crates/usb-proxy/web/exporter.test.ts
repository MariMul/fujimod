import test from 'node:test';
import assert from 'node:assert/strict';
import { Exporter } from './usb-exporter.ts';
import type { UsbDevice, UsbResult } from './webusb.ts';
import { Pipeline } from './pipeline.ts';
import type { Completion, Message, ExecuteMessage } from './types.ts';
function fixture() {
  const calls: (string | [string, number, number[]])[] = [];
  const alternate = {
    alternateSetting: 0,
    endpoints: [
      { endpointNumber: 2, direction: 'in', type: 'bulk', packetSize: 64 },
      { endpointNumber: 1, direction: 'out', type: 'bulk', packetSize: 64 },
    ],
  };
  const config = {
    configurationValue: 1,
    interfaces: [{ interfaceNumber: 0, alternates: [alternate], alternate }],
  };
  const device: UsbDevice = {
    vendorId: 0x04cb,
    productId: 0xff80,
    configurations: [config],
    async controlTransferIn() {
      throw Error('Unexpected control transfer');
    },
    async controlTransferOut() {
      throw Error('Unexpected control transfer');
    },
    async clearHalt() {
      throw Error('Unexpected clear halt');
    },
    async open() {
      calls.push('open');
    },
    async selectConfiguration() {
      this.configuration = config;
    },
    async claimInterface() {
      calls.push('claim');
    },
    async selectAlternateInterface() {},
    async close() {
      calls.push('close');
    },
    async transferIn() {
      calls.push('in');
      return { status: 'stall', data: new DataView(Uint8Array.of(1, 2, 3).buffer) };
    },
    async transferOut(endpoint: number, data: Uint8Array) {
      calls.push(['out', endpoint, [...data]]);
      return { status: 'ok', bytesWritten: data.length };
    },
  };
  const exporter = new Exporter({ getDevices: async () => [device] });
  return { exporter, device, calls };
}
const open = { op: 'open', configuration: 1, interface: 0, alternate: 0 };
await test('descriptors, binary payloads and partial failures survive', async () => {
  const { exporter, calls } = fixture();
  const [info] = await exporter.inventory();
  assert.equal(info.interfaces[0].endpoints[0].address, 0x82);
  await exporter.execute(info.id, open);
  assert.deepEqual(
    await exporter.execute(info.id, { op: 'transfer_in', endpoint: 0x82, length: 64 }),
    { data: Uint8Array.of(1, 2, 3), transferred: 3, error: 'stall' },
  );
  assert.equal(
    (
      await exporter.execute(info.id, {
        op: 'transfer_out',
        endpoint: 1,
        data: Uint8Array.of(0, 255, 128),
      })
    ).transferred,
    3,
  );
  assert.deepEqual(calls.at(-1), ['out', 1, [0, 255, 128]]);
  await exporter.stop();
  assert.equal(calls.at(-1), 'close');
});
await test('timed out USB promise cannot be overtaken by close', async () => {
  const { exporter, device, calls } = fixture();
  const [{ id }] = await exporter.inventory();
  await exporter.execute(id, open);
  let finish!: (result: UsbResult) => void;
  device.transferIn = () => {
    calls.push('in');
    return new Promise((resolve) => {
      finish = resolve;
    });
  };
  const pending = exporter.run(id, { op: 'transfer_in', endpoint: 0x82, length: 64 }, 10);
  assert.match((await pending).error!, /completion unknown/);
  assert.deepEqual(await exporter.run(id, { op: 'probe' }, 10), {});
  const closing = exporter.run(id, { op: 'close' }, 1000);
  assert.equal(calls.at(-1), 'in');
  finish({ status: 'ok', data: new DataView(new ArrayBuffer(0)) });
  await closing;
  assert.equal(calls.at(-1), 'close');
  await assert.rejects(exporter.execute(id, open), /fenced/);
});
await test('invalid endpoints never reach USB', async () => {
  const { exporter, calls } = fixture();
  const [{ id }] = await exporter.inventory();
  await exporter.execute(id, open);
  for (const endpoint of [0, 2, 0x92, 0x83, -1, 256]) {
    await assert.rejects(
      exporter.execute(id, { op: 'transfer_in', endpoint, length: 64 }),
      /endpoint|Endpoint/,
    );
  }
  assert.deepEqual(calls, ['open', 'claim']);
});

await test('a network probe requires no USB inventory or claim', async () => {
  const { exporter, calls } = fixture();
  assert.deepEqual(await exporter.run('', { op: 'probe' }, 10), {});
  assert.deepEqual(calls, []);
  assert.equal(exporter.claims.size, 0);
  assert.equal(exporter.fenced.size, 0);
});
await test('a pipeline runs in order, checks expectations and discards after a failure until sync', async () => {
  const ran: number[] = [];
  const pipeline = new Pipeline(async (m: ExecuteMessage): Promise<Completion> => {
    ran.push(m.seq);
    if (m.op.op === 'transfer_in') return { data: Uint8Array.of(m.seq, 0x5a, 3), transferred: 3 };
    return { transferred: m.op.data?.length ?? 0, error: m.op.endpoint === 3 ? 'stall' : null };
  });
  const execute = (seq: number, op: ExecuteMessage['op']): Message => ({
    type: 'execute',
    seq,
    device: '1',
    timeout_ms: 1000,
    op,
  });
  const read = (seq: number) => execute(seq, { op: 'transfer_in', endpoint: 0x82, length: 64 });
  const write = (seq: number, endpoint = 1) =>
    execute(seq, { op: 'transfer_out', endpoint, data: Uint8Array.of(seq) });
  const expect = (seq: number, of: number, bytes: number[], mask?: number[]): Message => ({
    type: 'expect',
    seq,
    of,
    offset: 1,
    bytes: Uint8Array.from(bytes),
    mask: mask && Uint8Array.from(mask),
  });
  const replies = [];
  for (const message of [
    read(1),
    expect(2, 1, [0x5a, 3]),
    expect(3, 1, [0x50], [0xf0]),
    write(4),
    expect(5, 1, [0x5b]),
    write(6),
    read(7),
    { type: 'sync', seq: 8 } as Message,
    write(9, 3),
    write(10),
    { type: 'sync', seq: 11 } as Message,
    write(12),
    expect(13, 99, [1]),
    { type: 'sync', seq: 14 } as Message,
  ])
    replies.push(await pipeline.handle(message));
  assert.deepEqual(ran, [1, 4, 9, 12]);
  assert.deepEqual(
    replies.map((r) => r && { ...r, completion: undefined }),
    [
      { type: 'result', seq: 1, completion: undefined },
      undefined,
      undefined,
      { type: 'result', seq: 4, completion: undefined },
      { type: 'expect_failed', seq: 5, actual: Uint8Array.of(0x5a), completion: undefined },
      undefined,
      undefined,
      { type: 'synced', seq: 8, discarded: 2, completion: undefined },
      { type: 'result', seq: 9, completion: undefined },
      undefined,
      { type: 'synced', seq: 11, discarded: 1, completion: undefined },
      { type: 'result', seq: 12, completion: undefined },
      { type: 'expect_failed', seq: 13, actual: new Uint8Array(), completion: undefined },
      { type: 'synced', seq: 14, discarded: 0, completion: undefined },
    ],
  );
});
