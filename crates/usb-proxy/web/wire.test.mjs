import test from 'node:test';
import assert from 'node:assert/strict';
import { VERSION, MAX_TRANSFER, MAX_HEADER, encodeFrame, decodeFrame } from './wire.js';
import { PeerChannel } from './peer.js';
const request = (data) => ({
  type: 'request',
  id: 7,
  device: 'camera',
  operation: { op: 'transfer_out', endpoint: 1, data },
  timeout_ms: 5000,
});
await test('opaque payloads survive with only metadata in JSON, including subarray offsets and the transfer limit', () => {
  for (const size of [0, 1, 256, 15999, 16000, 16001, MAX_TRANSFER]) {
    const storage = Uint8Array.from({ length: size + 17 }, (_, i) => i & 255);
    const data = storage.subarray(7, 7 + size),
      frame = request(data),
      wire = encodeFrame(frame);
    const headerSize = new DataView(wire.buffer).getUint32(4, true);
    const metadata = JSON.parse(new TextDecoder().decode(wire.subarray(8, 8 + headerSize)));
    assert.equal(metadata.operation.data, undefined);
    assert.equal(wire.length, 8 + headerSize + data.length);
    assert.deepEqual(decodeFrame(wire), frame);
    // Decoder exposes a view into the received frame rather than another payload copy.
    assert.equal(decodeFrame(wire).operation.data.buffer, wire.buffer);
  }
});
await test('binary completion preserves partial bytes and failure status', () => {
  const frame = {
    type: 'reply',
    id: 9,
    completion: { data: Uint8Array.of(0, 255, 128), transferred: 3, error: 'stall' },
  };
  assert.deepEqual(decodeFrame(encodeFrame(frame)), frame);
});
await test('the Rust/JS empty Probe wire format is identical', () => {
  const frame = {
    type: 'request',
    id: 7,
    device: '',
    operation: { op: 'probe' },
    timeout_ms: 5000,
  };
  const metadata =
    '{"type":"request","id":7,"device":"","operation":{"op":"probe"},"timeout_ms":5000}';
  const expected = Buffer.concat([
    Buffer.from([70, 85, VERSION, 0, Buffer.byteLength(metadata), 0, 0, 0]),
    Buffer.from(metadata),
  ]);
  assert.deepEqual(Buffer.from(encodeFrame(frame)), expected);
});
await test('invalid versions, truncated frames, excess payload and JSON payloads are rejected', () => {
  const wire = encodeFrame(request(Uint8Array.of(1, 2, 3)));
  assert.throws(() => decodeFrame('text'), /binary/);
  for (let i = 0; i < 8; i++) assert.throws(() => decodeFrame(wire.subarray(0, i)));
  const old = wire.slice();
  old[2] = 1;
  assert.throws(() => decodeFrame(old));
  const truncated = wire.slice();
  new DataView(truncated.buffer).setUint32(4, MAX_HEADER + 1, true);
  assert.throws(() => decodeFrame(truncated));
  assert.throws(() => encodeFrame(request(new Uint8Array(MAX_TRANSFER + 1))));
  assert.throws(() => encodeFrame(request('AQID')));
  const probe = encodeFrame({ type: 'request', operation: { op: 'probe' } });
  const extra = new Uint8Array(probe.length + 1);
  extra.set(probe);
  assert.throws(() => decodeFrame(extra));
  const smuggled = encodeFrame({ type: 'request', operation: { op: 'probe', data: [1, 2] } });
  assert.throws(() => decodeFrame(smuggled), /binary|payload/);
});
class Channel extends EventTarget {
  readyState = 'open';
  bufferedAmount = 0;
  packets = [];
  send(packet) {
    this.packets.push(packet);
  }
  close() {
    this.readyState = 'closed';
  }
}
await test('WebRTC chunks and reassembles binary without text conversion and honors backpressure', async () => {
  const tx = new Channel(),
    rx = new Channel();
  const sender = new PeerChannel(tx, new EventTarget()),
    receiver = new PeerChannel(rx, new EventTarget());
  const data = Uint8Array.from({ length: 48001 }, (_, i) => i & 255);
  tx.bufferedAmount = 300000;
  const sending = sender.send(data);
  assert.equal(tx.packets.length, 0);
  tx.bufferedAmount = 0;
  tx.dispatchEvent(new Event('bufferedamountlow'));
  await sending;
  assert.equal(tx.packets.length, 4);
  for (const packet of tx.packets) rx.onmessage({ data: packet.buffer });
  assert.deepEqual(await receiver.receive(), data);
  assert.ok(tx.packets.every((packet) => packet.length <= 16001));
});
