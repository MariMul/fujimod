import assert from 'node:assert/strict';
import test from 'node:test';
import { connectPeer, encodeUrl, gather, shareUrl, type Ticket } from './peer.ts';

class Connection extends EventTarget {
  iceGatheringState: RTCIceGatheringState;
  localDescription: RTCSessionDescription;
  constructor(sdp: string) {
    super();
    this.iceGatheringState = 'gathering';
    this.localDescription = { sdp, type: 'offer', toJSON: () => ({ sdp, type: 'offer' }) };
  }
}
const host = 'v=0\r\na=candidate:1 1 udp 1 10.0.0.1 5000 typ host\r\n';

await test('gathering that never completes sends the candidates found so far', async () => {
  await gather(new Connection(host), { settle: 10, limit: 100 });
});

await test('gathering with no candidates fails at the limit', async () => {
  await assert.rejects(
    gather(new Connection('v=0\r\n'), { settle: 10, limit: 50 }),
    /no candidates/,
  );
});

await test('completed gathering resolves at once', async () => {
  const connection = new Connection('v=0\r\n');
  setTimeout(() => {
    connection.iceGatheringState = 'complete';
    connection.dispatchEvent(new Event('icegatheringstatechange'));
  }, 5);
  await gather(connection, { settle: 1000, limit: 2000 });
});

await test('host_offline leaves an established controller peer connected', async () => {
  class Channel {
    readyState: RTCDataChannelState = 'connecting';
    onopen: ((event: Event) => void) | null = null;
    closeCalls = 0;
    close() {
      this.closeCalls++;
      this.readyState = 'closed';
    }
  }
  class PeerConnection extends EventTarget {
    static current?: PeerConnection;
    channel?: Channel;
    connectionState: RTCPeerConnectionState = 'new';
    iceGatheringState: RTCIceGatheringState = 'complete';
    localDescription: RTCSessionDescription | null = null;
    closeCalls = 0;
    constructor() {
      super();
      PeerConnection.current = this;
    }
    createDataChannel() {
      this.channel = new Channel();
      return this.channel as unknown as RTCDataChannel;
    }
    async createOffer(): Promise<RTCSessionDescriptionInit> {
      return { type: 'offer', sdp: 'v=0\r\na=candidate:1 1 udp 1 10.0.0.1 5000 typ host\r\n' };
    }
    async setLocalDescription(description: RTCSessionDescriptionInit) {
      this.localDescription = {
        ...description,
        toJSON: () => description,
      } as RTCSessionDescription;
      this.channel!.readyState = 'open';
      this.channel!.onopen?.(new Event('open'));
    }
    async setRemoteDescription() {}
    setConfiguration() {}
    close() {
      this.closeCalls++;
      this.connectionState = 'closed';
    }
  }
  class WebSocket {
    static current?: WebSocket;
    readyState = 0;
    onopen: ((event: Event) => void) | null = null;
    onmessage: ((event: { data: string }) => void) | null = null;
    onerror: (() => void) | null = null;
    onclose: (() => void) | null = null;
    closeCalls = 0;
    constructor() {
      WebSocket.current = this;
      queueMicrotask(() => {
        this.readyState = 1;
        this.onopen?.(new Event('open'));
      });
    }
    send(value: string) {
      if ((JSON.parse(value) as { type: string }).type === 'join')
        queueMicrotask(() =>
          this.onmessage?.({ data: JSON.stringify({ type: 'ready', peer: 'peer-1' }) }),
        );
    }
    receive(value: object) {
      this.onmessage?.({ data: JSON.stringify(value) });
    }
    close() {
      this.closeCalls++;
      this.readyState = 3;
    }
  }
  const websocketDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'WebSocket');
  const peerDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'RTCPeerConnection');
  Object.defineProperty(globalThis, 'WebSocket', { configurable: true, value: WebSocket });
  Object.defineProperty(globalThis, 'RTCPeerConnection', {
    configurable: true,
    value: PeerConnection,
  });
  let peer: Awaited<ReturnType<typeof connectPeer>> | undefined;
  try {
    const bytes = encodeUrl(new Uint8Array(32).fill(1));
    const ticket: Ticket = {
      v: 1,
      signal: 'wss://signal.example',
      room: bytes,
      key: bytes,
      stun: [],
    };
    peer = await connectPeer(shareUrl('https://example.com/', ticket));
    WebSocket.current!.receive({ type: 'host_offline' });
    assert.equal(peer.ended, false);
    assert.equal(PeerConnection.current!.channel!.readyState, 'open');
    assert.equal(PeerConnection.current!.channel!.closeCalls, 0);
    assert.equal(PeerConnection.current!.closeCalls, 0);
    assert.equal(WebSocket.current!.closeCalls, 0);
  } finally {
    peer?.close();
    if (websocketDescriptor) Object.defineProperty(globalThis, 'WebSocket', websocketDescriptor);
    else Reflect.deleteProperty(globalThis, 'WebSocket');
    if (peerDescriptor) Object.defineProperty(globalThis, 'RTCPeerConnection', peerDescriptor);
    else Reflect.deleteProperty(globalThis, 'RTCPeerConnection');
  }
});
