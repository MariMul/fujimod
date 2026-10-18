// Exercises ShareHost with isolated signaling and data channels. No network or USB access.
import test from 'node:test';
import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { VERSION, encodeFrame, decodeFrame } from './wire.js';
import { ShareHost, newTicket } from './peer.js';
class Socket {
  constructor() {
    queueMicrotask(() => this.onopen?.());
  }
  send(text) {
    if (JSON.parse(text).type === 'join')
      queueMicrotask(() => this.onmessage({ data: '{"type":"ready"}' }));
  }
  close() {
    this.onclose?.();
  }
}
class Connection extends EventTarget {
  static all = [];
  constructor() {
    super();
    this.iceGatheringState = 'complete';
    this.connectionState = 'connected';
    Connection.all.push(this);
  }
  async setRemoteDescription() {}
  async createAnswer() {
    return { type: 'answer', sdp: 'v=0' };
  }
  async setLocalDescription(answer) {
    this.localDescription = { ...answer, toJSON: () => answer };
  }
  close() {}
}
class Channel extends EventTarget {
  readyState = 'open';
  bufferedAmount = 0;
  sent = [];
  failReplies = false;
  send(packet) {
    const frame = decodeFrame(packet.subarray(1));
    if (this.failReplies && frame.type === 'reply') throw Error('Data channel send failed');
    this.sent.push(frame);
  }
  close() {
    if (this.readyState === 'closed') return;
    this.readyState = 'closed';
    this.onclose?.();
    this.dispatchEvent(new Event('close'));
  }
  input(frame) {
    const bytes = encodeFrame(frame),
      packet = new Uint8Array(bytes.length + 1);
    packet[0] = 1;
    packet.set(bytes, 1);
    this.onmessage({ data: packet.buffer });
  }
}
async function until(predicate) {
  for (let i = 0; i < 200; i++) {
    if (predicate()) return;
    await delay(1);
  }
  assert.ok(predicate(), 'ShareHost did not complete');
}
async function offer(ticket, id) {
  const iv = new Uint8Array(12);
  const key = await crypto.subtle.importKey(
    'raw',
    Buffer.from(ticket.key, 'base64url'),
    'AES-GCM',
    false,
    ['encrypt'],
  );
  const encrypted = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv, additionalData: new TextEncoder().encode(ticket.room + '/' + id) },
    key,
    new TextEncoder().encode('{"type":"offer","sdp":"v=0"}'),
  );
  return Buffer.concat([iv, new Uint8Array(encrypted)]).toString('base64url');
}
async function fixture(observer) {
  const oldSocket = globalThis.WebSocket,
    oldConnection = globalThis.RTCPeerConnection;
  globalThis.WebSocket = Socket;
  globalThis.RTCPeerConnection = Connection;
  const calls = [],
    errors = [],
    events = [],
    ticket = newTicket('wss://isolated.test/signal', []);
  const device = {
    descriptor: { id: 'camera' },
    execute: async (op) => {
      calls.push(op.op);
      return op.op === 'transfer_in'
        ? { data: Uint8Array.of(1, 2, 3), transferred: 3 }
        : { transferred: 12 };
    },
    close: async () => {},
  };
  const host = await ShareHost.create(ticket, device, {
    traffic: (event) => {
      events.push(event);
      observer?.(event);
    },
    error: (error) => errors.push(error),
  });
  await host.message({
    type: 'signal',
    from: 'controller',
    data: await offer(ticket, 'controller'),
  });
  const connection = Connection.all.at(-1),
    channel = new Channel();
  connection.ondatachannel({ channel });
  channel.input({
    type: 'hello',
    role: 'controller',
    version: VERSION,
    token: ticket.key,
    reason: 'isolated test',
  });
  await until(() => channel.sent.some((frame) => frame.type === 'ready'));
  return {
    channel,
    events,
    calls,
    errors,
    async request(id, operation) {
      channel.input({ type: 'request', id, device: 'camera', operation, timeout_ms: 5000 });
      await until(() => channel.sent.some((frame) => frame.id === id));
      return channel.sent.find((frame) => frame.id === id).completion;
    },
    dispose() {
      host.stop();
      globalThis.WebSocket = oldSocket;
      globalThis.RTCPeerConnection = oldConnection;
    },
  };
}
await test('ShareHost reports every request/completion, including ownership failures', async () => {
  const f = await fixture();
  try {
    await f.request(1, { op: 'list' });
    await f.request(2, { op: 'probe' });
    await f.request(3, { op: 'open' });
    assert.deepEqual(
      (await f.request(4, { op: 'transfer_in', endpoint: 0x82, length: 64 })).data,
      Uint8Array.of(1, 2, 3),
    );
    await f.request(5, { op: 'close' });
    const failed = await f.request(6, { op: 'transfer_in', endpoint: 0x82, length: 64 });
    assert.match(failed.error, /not claimed/);
    assert.deepEqual(f.calls, ['open', 'transfer_in', 'close']);
    assert.equal(f.events.length, 12);
    for (let i = 0; i < 6; i++) {
      assert.equal(f.events[i * 2].phase, 'submitted');
      assert.equal(f.events[i * 2 + 1].phase, 'completed');
      assert.equal(f.events[i * 2].id, i + 1);
      assert.equal(f.events[i * 2].controller, 'controller');
    }
    assert.match(f.events.at(-1).completion.error, /not claimed/);
  } finally {
    f.dispose();
  }
});
await test('ShareHost still replies when observers throw and reports completion before a send fails', async () => {
  const f = await fixture(() => {
    throw Error('Observer failed');
  });
  try {
    await f.request(1, { op: 'open' });
    assert.equal(f.errors.length, 0);
    f.channel.failReplies = true;
    f.channel.input({
      type: 'request',
      id: 2,
      device: 'camera',
      operation: { op: 'transfer_in', endpoint: 0x82, length: 64 },
      timeout_ms: 5000,
    });
    await until(() => f.channel.readyState === 'closed');
    assert.equal(f.events.at(-1).phase, 'completed');
    assert.equal(f.events.at(-1).completion.transferred, 3);
    assert.match(f.errors[0].message, /send failed/);
  } finally {
    f.dispose();
  }
});
