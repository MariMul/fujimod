import { readFile } from 'node:fs/promises';
import test from 'node:test';
import assert from 'node:assert/strict';
const { Exporter } = await import('data:text/javascript;base64,' + Buffer.from(await readFile(new URL('./usb-exporter.js', import.meta.url))).toString('base64'));
function fixture() {
  const calls = [];
  const alternate = { alternateSetting: 0, endpoints: [{ endpointNumber: 2, direction: 'in', type: 'bulk', packetSize: 64 }, { endpointNumber: 1, direction: 'out', type: 'bulk', packetSize: 64 }] };
  const config = { configurationValue: 1, interfaces: [{ interfaceNumber: 0, alternates: [alternate], alternate }] };
  const device = { vendorId: 0x04cb, productId: 0xff80, configurations: [config],
    async open() { calls.push('open'); }, async selectConfiguration() { this.configuration = config; },
    async claimInterface() { calls.push('claim'); }, async selectAlternateInterface() {},
    async close() { calls.push('close'); },
    async transferIn() { calls.push('in'); return { status: 'stall', data: new DataView(Uint8Array.of(1, 2, 3).buffer) }; },
    async transferOut(endpoint, data) { calls.push(['out', endpoint, [...data]]); return { status: 'ok', bytesWritten: data.length }; },
  };
  const exporter = new Exporter({ getDevices: async () => [device] });
  return { exporter, device, calls };
}
const open = { op: 'open', configuration: 1, interface: 0, alternate: 0 };
test('descriptors, binary payloads and partial failures survive', async () => {
  const { exporter, calls } = fixture();
  const [info] = await exporter.inventory();
  assert.equal(info.interfaces[0].endpoints[0].address, 0x82);
  await exporter.execute(info.id, open);
  assert.deepEqual(await exporter.execute(info.id, { op: 'transfer_in', endpoint: 0x82, length: 64 }), { data: 'AQID', transferred: 3, error: 'stall' });
  assert.equal((await exporter.execute(info.id, { op: 'transfer_out', endpoint: 1, data: 'AP+A' })).transferred, 3);
  assert.deepEqual(calls.at(-1), ['out', 1, [0, 255, 128]]);
  await exporter.stop(); assert.equal(calls.at(-1), 'close');
});
test('timed out USB promise cannot be overtaken by close', async () => {
  const { exporter, device, calls } = fixture();
  const [{ id }] = await exporter.inventory(); await exporter.execute(id, open);
  let finish, timeoutReply;
  device.transferIn = () => { calls.push('in'); return new Promise((resolve) => { finish = resolve; }); };
  const replies = [];
  const timedOut = new Promise((resolve) => { timeoutReply = resolve; });
  exporter.onreply = (reply) => { replies.push(reply); if (reply.id === 1) timeoutReply(); };
  const pending = exporter.request({ id: 1, device: id, timeout_ms: 10, operation: { op: 'transfer_in', endpoint: 0x82, length: 64 } });
  await timedOut;
  assert.match(replies[0].completion.error, /completion unknown/);
  const closing = exporter.request({ id: 2, device: id, timeout_ms: 1000, operation: { op: 'close' } });
  assert.equal(calls.at(-1), 'in');
  finish({ status: 'ok', data: new DataView(new ArrayBuffer(0)) });
  await pending; await closing;
  assert.equal(calls.at(-1), 'close'); assert.equal(replies.length, 2);
  await assert.rejects(exporter.execute(id, open), /fenced/);
});
test('invalid endpoints never reach USB', async () => {
  const { exporter, calls } = fixture(); const [{ id }] = await exporter.inventory();
  await exporter.execute(id, open);
  for (const endpoint of [0, 2, 0x92, 0x83, -1, 256]) {
    await assert.rejects(exporter.execute(id, { op: 'transfer_in', endpoint, length: 64 }), /endpoint|Endpoint/);
  }
  assert.deepEqual(calls, ['open', 'claim']);
});
