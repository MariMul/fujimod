// WebRTC USB transport. Only encrypted SDP crosses the rendezvous service.
const MAX = (8 * 1024 * 1024 * 4) / 3 + 65536;
const encoder = new TextEncoder();
export const encodeUrl = (bytes) => {
  let text = '';
  for (let i = 0; i < bytes.length; i += 32768)
    text += String.fromCharCode(...bytes.subarray(i, i + 32768));
  return btoa(text).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
};
const decodeUrl = (text) =>
  Uint8Array.from(atob(text.replaceAll('-', '+').replaceAll('_', '/')), (c) => c.charCodeAt(0));
const random = () => encodeUrl(crypto.getRandomValues(new Uint8Array(32)));
export function newTicket(signal, stun = ['stun:stun.cloudflare.com:3478']) {
  return { v: 1, signal, room: random(), key: random(), stun };
}
export function parseShare(url) {
  const values = new URLSearchParams(new URL(url).hash.slice(1).replace(/^\?/, '')).getAll(
    'remotecontrol',
  );
  const [value] = values;
  if (values.length !== 1 || !value || value.length > 4096)
    throw Error('Invalid camera share link');
  const ticket = JSON.parse(new TextDecoder().decode(decodeUrl(value)));
  const signal = new URL(ticket.signal);
  if (
    ticket.v !== 1 ||
    decodeUrl(ticket.key).length !== 32 ||
    decodeUrl(ticket.room).length !== 32 ||
    !/^wss?:$/.test(signal.protocol) ||
    signal.username ||
    signal.password ||
    !Array.isArray(ticket.stun) ||
    ticket.stun.length > 4 ||
    ticket.stun.some(
      (server) => typeof server !== 'string' || !server.startsWith('stun:') || server.length > 256,
    )
  )
    throw Error('Invalid camera share link');
  return ticket;
}
export function shareUrl(current, ticket) {
  const url = new URL(current);
  url.hash = '?remotecontrol=' + encodeUrl(encoder.encode(JSON.stringify(ticket)));
  return url.href;
}
async function cryptKey(ticket) {
  return crypto.subtle.importKey('raw', decodeUrl(ticket.key), 'AES-GCM', false, [
    'encrypt',
    'decrypt',
  ]);
}
async function seal(ticket, peer, value) {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const data = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv, additionalData: encoder.encode(ticket.room + '/' + peer) },
      await cryptKey(ticket),
      encoder.encode(JSON.stringify(value)),
    ),
  );
  const result = new Uint8Array(12 + data.length);
  result.set(iv);
  result.set(data, 12);
  return encodeUrl(result);
}
async function unseal(ticket, peer, value) {
  const bytes = decodeUrl(value);
  const data = await crypto.subtle.decrypt(
    {
      name: 'AES-GCM',
      iv: bytes.slice(0, 12),
      additionalData: encoder.encode(ticket.room + '/' + peer),
    },
    await cryptKey(ticket),
    bytes.slice(12),
  );
  return JSON.parse(new TextDecoder().decode(data));
}
function signaling(ticket, role, message, ended) {
  const url = new URL(ticket.signal);
  url.searchParams.set('room', ticket.room);
  const socket = new WebSocket(url);
  const ready = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      socket.close();
      reject(Error('Signaling connection timed out'));
    }, 10000);
    socket.onopen = () => socket.send(JSON.stringify({ type: 'join', room: ticket.room, role }));
    socket.onmessage = ({ data }) => {
      const frame = JSON.parse(data);
      if (frame.type === 'ready') {
        clearTimeout(timer);
        resolve(frame);
      } else if (frame.type === 'error') {
        clearTimeout(timer);
        const error = Error(frame.message || 'Camera share unavailable');
        reject(error);
        ended(error);
        socket.close();
      } else if (frame.type === 'ended') {
        socket.close();
        ended(Error('Camera sharing ended'));
      } else Promise.resolve(message(frame)).catch(ended);
    };
    socket.onerror = () => {
      clearTimeout(timer);
      reject(Error('Signaling service unavailable'));
    };
    socket.onclose = () => {
      clearTimeout(timer);
      reject(Error('Camera share unavailable'));
      ended(Error('Signaling connection closed'));
    };
  });
  return {
    ready,
    send: (frame) => socket.send(JSON.stringify(frame)),
    close: () => socket.close(),
  };
}
export async function gather(pc) {
  if (pc.iceGatheringState === 'complete') return;
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pc.removeEventListener('icegatheringstatechange', changed);
      reject(Error('ICE gathering timed out'));
    }, 12000);
    function changed() {
      if (pc.iceGatheringState === 'complete') {
        clearTimeout(timer);
        pc.removeEventListener('icegatheringstatechange', changed);
        resolve();
      }
    }
    pc.addEventListener('icegatheringstatechange', changed);
  });
}
// Ordered, bounded framing handles USB requests larger than a single SCTP message.
export class PeerChannel {
  constructor(channel, pc) {
    this.channel = channel;
    this.pc = pc;
    this.queue = [];
    this.waiters = [];
    this.parts = [];
    this.size = 0;
    this.ended = false;
    channel.binaryType = 'arraybuffer';
    channel.bufferedAmountLowThreshold = 131072;
    channel.onmessage = ({ data }) => {
      const chunk = new Uint8Array(data);
      if (chunk.length < 1 || chunk[0] > 1 || (this.size += chunk.length - 1) > MAX) {
        this.close();
        return;
      }
      this.parts.push(chunk.subarray(1));
      if (chunk[0] === 1) {
        const bytes = new Uint8Array(this.size);
        let offset = 0;
        for (const part of this.parts) {
          bytes.set(part, offset);
          offset += part.length;
        }
        this.parts = [];
        this.size = 0;
        const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
        const waiter = this.waiters.shift();
        if (waiter) waiter.resolve(text);
        else this.queue.push(text);
        if (this.queue.length > 16) this.close();
      }
    };
    channel.onclose = () => this.close();
    channel.onerror = () => this.close();
    pc.addEventListener('connectionstatechange', () => {
      if (['failed', 'closed'].includes(pc.connectionState)) this.close();
    });
  }
  async send(text) {
    const bytes = encoder.encode(text);
    if (bytes.length > MAX) throw Error('USB message exceeds limit');
    for (let offset = 0; offset < bytes.length; offset += 16000) {
      if (this.channel.bufferedAmount > 262144)
        await new Promise((resolve, reject) => {
          const low = () => {
              cleanup();
              resolve();
            },
            close = () => {
              cleanup();
              reject(Error('Peer disconnected; completion unknown'));
            };
          const timer = setTimeout(close, 10000);
          const cleanup = () => {
            clearTimeout(timer);
            this.channel.removeEventListener('bufferedamountlow', low);
            this.channel.removeEventListener('close', close);
          };
          this.channel.addEventListener('bufferedamountlow', low, { once: true });
          this.channel.addEventListener('close', close, { once: true });
        });
      if (this.ended || this.channel.readyState !== 'open')
        throw Error('Peer disconnected; completion unknown');
      const piece = bytes.subarray(offset, offset + 16000),
        packet = new Uint8Array(piece.length + 1);
      packet[0] = offset + piece.length === bytes.length ? 1 : 0;
      packet.set(piece, 1);
      this.channel.send(packet);
    }
  }
  receive() {
    if (this.queue.length) return Promise.resolve(this.queue.shift());
    if (this.ended) return Promise.reject(Error('Peer disconnected; completion unknown'));
    return new Promise((resolve, reject) => this.waiters.push({ resolve, reject }));
  }
  close() {
    if (this.ended) return;
    this.ended = true;
    for (const waiter of this.waiters.splice(0))
      waiter.reject(Error('Peer disconnected; completion unknown'));
    this.parts = [];
    this.queue = [];
    this.size = 0;
    this.channel.close();
    this.pc.close();
    this.onclose?.();
  }
}
function configuration(stun) {
  return { iceServers: stun.map((urls) => ({ urls })) };
}
export async function connectPeer(url) {
  const ticket = parseShare(url),
    pc = new RTCPeerConnection(configuration(ticket.stun || []));
  const channel = pc.createDataChannel('fujimod-usb', { ordered: true });
  const peer = new PeerChannel(channel, pc);
  let rejectOpen;
  const opened = new Promise((resolve, reject) => {
    rejectOpen = reject;
    channel.onopen = () => resolve();
  });
  opened.catch(() => {});
  const signal = signaling(
    ticket,
    'client',
    async (frame) => {
      if (frame.type !== 'signal' || frame.from !== 'host') return;
      const answer = await unseal(ticket, id, frame.data);
      if (answer.type !== 'answer') throw Error('Invalid peer answer');
      await pc.setRemoteDescription(answer);
    },
    (error) => {
      if (channel.readyState !== 'open') {
        rejectOpen(error);
        peer.close();
      }
    },
  );
  const timer = setTimeout(() => {
    rejectOpen(Error('Direct connection unavailable'));
    peer.close();
  }, 25000);
  let id;
  try {
    const ready = await signal.ready;
    id = ready.peer;
    if (ready.ice_servers) pc.setConfiguration({ iceServers: ready.ice_servers });
    await pc.setLocalDescription(await pc.createOffer());
    await gather(pc);
    signal.send({ type: 'signal', data: await seal(ticket, id, pc.localDescription.toJSON()) });
    await opened;
    peer.onclose = () => signal.close();
    return peer;
  } catch (error) {
    signal.close();
    peer.close();
    throw error;
  } finally {
    clearTimeout(timer);
  }
}
export class ShareHost {
  static async create(ticket, device, callbacks = {}) {
    const host = new ShareHost(ticket, device, callbacks);
    try {
      const ready = await host.signal.ready;
      host.iceServers = ready.ice_servers || configuration(ticket.stun || []).iceServers;
      return host;
    } catch (error) {
      host.stop();
      throw error;
    }
  }
  constructor(ticket, device, callbacks) {
    this.ticket = ticket;
    this.device = device;
    this.callbacks = callbacks;
    this.peers = new Map();
    this.owner = undefined;
    this.closed = false;
    this.fenced = false;
    this.signal = signaling(
      ticket,
      'host',
      (frame) =>
        this.message(frame).catch(() => {
          this.peers.get(frame.from)?.close();
          this.peers.delete(frame.from);
        }),
      (error) => {
        if (!this.closed) {
          this.callbacks.error?.(error);
          this.stop();
        }
      },
    );
  }
  async message(frame) {
    if (this.closed) return;
    if (frame.type === 'left') {
      this.peers.get(frame.peer)?.close();
      return;
    }
    if (frame.type !== 'signal' || this.peers.has(frame.from)) return;
    const offer = await unseal(this.ticket, frame.from, frame.data);
    if (offer.type !== 'offer') throw Error('Invalid peer offer');
    const pc = new RTCPeerConnection({ iceServers: this.iceServers });
    this.peers.set(frame.from, { close: () => pc.close() });
    const timer = setTimeout(() => {
      if (pc.connectionState !== 'connected') {
        pc.close();
        this.peers.delete(frame.from);
      }
    }, 25000);
    pc.ondatachannel = ({ channel }) => {
      const peer = new PeerChannel(channel, pc);
      this.peers.set(frame.from, peer);
      let authenticated = false;
      peer.onclose = () => {
        clearTimeout(timer);
        this.peers.delete(frame.from);
        if (authenticated) this.callbacks.disconnected?.(frame.from);
        if (this.owner === frame.from) {
          this.owner = undefined;
          this.fenced = true;
          if (!this.closed)
            this.callbacks.error?.(
              Error('Remote controller disconnected while USB was claimed; completion unknown'),
            );
          void this.device.close().catch((error) => this.callbacks.error?.(error));
        }
      };
      void (async () => {
        const hello = JSON.parse(await peer.receive());
        if (
          hello.type !== 'hello' ||
          hello.version !== 1 ||
          hello.role !== 'controller' ||
          hello.token !== this.ticket.key
        )
          throw Error('Invalid peer authentication');
        authenticated = true;
        clearTimeout(timer);
        this.callbacks.connected?.(frame.from);
        await peer.send(JSON.stringify({ type: 'ready', version: 1, id: frame.from }));
        while (!peer.ended) {
          const request = JSON.parse(await peer.receive());
          if (
            request.type !== 'request' ||
            !Number.isSafeInteger(request.id) ||
            request.timeout_ms < 1 ||
            request.timeout_ms > 120000
          )
            throw Error('Invalid USB request');
          let completion = { devices: [], data: '', transferred: 0, error: null };
          try {
            if (request.operation.op === 'probe') { /* Network echo; never touches USB. */ }
            else if (request.operation.op === 'list') completion.devices = [this.device.descriptor];
            else {
              if (request.device !== this.device.descriptor.id)
                throw Error('Unknown shared device');
              if (request.operation.op === 'open') {
                if (this.fenced)
                  throw Error('Shared camera completion unknown; reconnect on the host');
                if (this.owner !== undefined) throw Error('Shared camera is in use');
                this.owner = frame.from;
              } else if (this.owner !== frame.from) throw Error('Shared camera is not claimed');
              Object.assign(
                completion,
                await this.device.execute(request.operation, request.timeout_ms),
              );
              if (request.operation.op === 'close' && !completion.error) this.owner = undefined;
            }
          } catch (error) {
            completion.error = error.message || String(error);
          }
          await peer.send(JSON.stringify({ type: 'reply', id: request.id, completion }));
          if (completion.error) {
            this.callbacks.error?.(Error(completion.error));
            peer.close();
            break;
          }
        }
      })().catch((error) => {
        if (!peer.ended) this.callbacks.error?.(error);
        peer.close();
      });
    };
    await pc.setRemoteDescription(offer);
    await pc.setLocalDescription(await pc.createAnswer());
    await gather(pc);
    this.signal.send({
      type: 'signal',
      to: frame.from,
      data: await seal(this.ticket, frame.from, pc.localDescription.toJSON()),
    });
  }
  stop() {
    if (this.closed) return;
    this.closed = true;
    this.signal.close();
    for (const peer of this.peers.values()) peer.close();
    this.peers.clear();
    void this.device.close().catch((error) => this.callbacks.error?.(error));
  }
}
