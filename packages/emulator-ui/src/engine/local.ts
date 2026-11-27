import { parseShare } from '../../../../crates/usb-proxy/web/peer.ts';
import type { Frame } from './api';

const HEADER_SIZE = 16;

export function localEmulatorUrl(endpoint: string): URL {
  const { signal, key } = parseShare(endpoint);
  const url = new URL(signal);
  if (url.protocol !== 'ws:' || url.hostname !== '127.0.0.1') {
    throw Error('The native emulator requires a loopback connection.');
  }
  url.pathname = '/emulator/' + encodeURIComponent(key);
  url.search = '';
  url.hash = '';
  return url;
}

function lcdFrame(bytes: ArrayBuffer): Frame {
  if (bytes.byteLength < HEADER_SIZE) throw Error('Incomplete native LCD frame.');
  const header = new DataView(bytes);
  const width = header.getUint32(0);
  const height = header.getUint32(4);
  const sequence = Number(header.getBigUint64(8));
  if (!width || !height || bytes.byteLength - HEADER_SIZE !== width * height * 4) {
    throw Error('Invalid native LCD frame size.');
  }
  return { width, height, sequence, rgba: new Uint8ClampedArray(bytes, HEADER_SIZE) };
}

export async function connectLocal(
  endpoint: string,
  receive: (message: string) => void,
  show: (frame: Frame) => void,
  fail: (error: Error) => void,
) {
  const socket = new WebSocket(localEmulatorUrl(endpoint));
  socket.binaryType = 'arraybuffer';
  const closed = Error('Native emulator connection closed.');
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onclose = () => reject(closed);
  });
  socket.onclose = () => fail(closed);
  socket.onmessage = ({ data }: MessageEvent<string | ArrayBuffer>) => {
    try {
      if (typeof data === 'string') receive(data);
      else show(lcdFrame(data));
    } catch (error) {
      fail(error instanceof Error ? error : Error(String(error)));
      socket.close();
    }
  };
  return {
    async send(bytes: Uint8Array<ArrayBuffer>) {
      if (socket.readyState !== WebSocket.OPEN) throw closed;
      socket.send(bytes);
    },
    close: () => socket.close(),
  };
}
