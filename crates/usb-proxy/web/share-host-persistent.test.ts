import assert from 'node:assert/strict';
import test, { mock } from 'node:test';
import { exporterFixture } from './exporter-fixture.ts';
import { REJOIN_MS, ShareHost, newOwnerKey, newTicket } from './peer.ts';

// A scripted signaling service: each socket answers its join with the next reply.
class Socket {
  static sockets: Socket[] = [];
  static replies: object[] = [];
  sent: { type: string; owner_key?: string }[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  closed = false;
  constructor() {
    Socket.sockets.push(this);
    queueMicrotask(() => this.onopen?.());
  }
  send(value: string) {
    const frame = JSON.parse(value) as { type: string; owner_key?: string };
    this.sent.push(frame);
    if (frame.type === 'join') {
      const reply = Socket.replies.shift() ?? { type: 'ready', peer: 'host' };
      queueMicrotask(() => this.receive(reply));
    }
  }
  receive(frame: object) {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }
  /** The service dropped the connection. */
  drop() {
    this.closed = true;
    this.onclose?.();
  }
  close() {
    this.closed = true;
  }
}
const device = () =>
  exporterFixture({
    id: 'cam',
    vendor_id: 0,
    product_id: 0,
    manufacturer: null,
    product: null,
    serial: null,
    interfaces: [],
  });
const settle = () => new Promise<void>((resolve) => setImmediate(resolve));

const previous = Object.getOwnPropertyDescriptor(globalThis, 'WebSocket');
Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: Socket });
mock.timers.enable({ apis: ['setTimeout'] });
try {
  await test('a persistent owner joins with its key and rejoins when the connection drops', async () => {
    Socket.sockets = [];
    Socket.replies = [{ type: 'error', message: 'Room already has an owner' }];
    const events: string[] = [];
    const ownerKey = newOwnerKey();
    const creating = ShareHost.create(
      newTicket('wss://signal.example/usb-proxy/signal'),
      device(),
      { signaling: (connected, reason) => events.push(`${connected} ${reason ?? ''}`.trim()) },
      ownerKey,
    );
    await settle();
    assert.deepEqual(events, ['false Room already has an owner']);
    mock.timers.tick(REJOIN_MS);
    const host = await creating;
    assert.equal(Socket.sockets.length, 2);
    assert.ok(Socket.sockets.every((socket) => socket.sent[0].owner_key === ownerKey));

    Socket.sockets[1].drop();
    assert.equal(host.closed, false);
    await settle();
    assert.equal(Socket.sockets.length, 3);
    await settle();
    assert.equal(events.at(-1), 'true');
    assert.equal(host.live, true);

    await host.end();
    assert.deepEqual(Socket.sockets[2].sent.at(-1), { type: 'end' });
    assert.equal(Socket.sockets[2].closed, true);
  });

  await test('a revoked room ends the owner for good', async () => {
    Socket.sockets = [];
    Socket.replies = [];
    let revoked = 0;
    const errors: string[] = [];
    const host = await ShareHost.create(
      newTicket('wss://signal.example/usb-proxy/signal'),
      device(),
      { revoked: () => revoked++, error: (error) => errors.push(error.message) },
      newOwnerKey(),
    );
    Socket.sockets[0].receive({ type: 'ended' });
    await settle();
    assert.equal(revoked, 1);
    assert.deepEqual(errors, ['Camera sharing ended']);
    assert.equal(host.closed, true);
    assert.equal(Socket.sockets.length, 1);
  });

  await test('a refused owner key is not retried', async () => {
    Socket.sockets = [];
    Socket.replies = [{ type: 'error', message: 'Invalid owner key' }];
    let revoked = 0;
    await assert.rejects(
      ShareHost.create(
        newTicket('wss://signal.example/usb-proxy/signal'),
        device(),
        { revoked: () => revoked++ },
        newOwnerKey(),
      ),
      /Invalid owner key/,
    );
    assert.equal(revoked, 1);
    assert.equal(Socket.sockets.length, 1);
  });

  await test('an owner that is away joins once to end the room', async () => {
    Socket.sockets = [];
    Socket.replies = [];
    const host = await ShareHost.create(
      newTicket('wss://signal.example/usb-proxy/signal'),
      device(),
      {},
      newOwnerKey(),
    );
    Socket.replies = [{ type: 'error', message: 'Signaling service unavailable' }];
    Socket.sockets[0].drop();
    await settle();
    assert.equal(host.live, false);
    await host.end();
    assert.deepEqual(Socket.sockets.at(-1)!.sent.at(-1), { type: 'end' });
    assert.equal(host.closed, true);
  });
} finally {
  mock.timers.reset();
  if (previous) Object.defineProperty(globalThis, 'WebSocket', previous);
  else Reflect.deleteProperty(globalThis, 'WebSocket');
}
