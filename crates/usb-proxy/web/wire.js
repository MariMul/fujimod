// Version 2: FU 02 00, little-endian metadata length, UTF-8 JSON metadata,
// then opaque payload bytes. Only the bounded metadata goes through a text codec.
export const VERSION = 2;
export const MAX_TRANSFER = 8 * 1024 * 1024;
export const MAX_HEADER = 65536;
export const MAX_MESSAGE = 8 + MAX_HEADER + MAX_TRANSFER;
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const empty = new Uint8Array();
function payloadOwner(frame) {
  if (frame.type === 'reply') return frame.completion;
  if (frame.type === 'request' && ['control_out', 'transfer_out'].includes(frame.operation?.op))
    return frame.operation;
}
export function encodeFrame(frame) {
  const owner = payloadOwner(frame), data = owner?.data ?? empty;
  if (!(data instanceof Uint8Array) || data.length > MAX_TRANSFER)
    throw Error('Invalid binary USB payload');
  // Omit the raw bytes without traversing or copying them into JSON arrays.
  const metadata = owner
    ? frame.type === 'reply'
      ? { ...frame, completion: { ...owner, data: undefined } }
      : { ...frame, operation: { ...owner, data: undefined } }
    : frame;
  const header = encoder.encode(JSON.stringify(metadata));
  if (header.length > MAX_HEADER) throw Error('Frame header exceeds limit');
  const bytes = new Uint8Array(8 + header.length + data.length);
  bytes.set([70, 85, VERSION, 0]);
  new DataView(bytes.buffer).setUint32(4, header.length, true);
  bytes.set(header, 8);
  bytes.set(data, 8 + header.length);
  return bytes;
}
export function decodeFrame(input) {
  const bytes = input instanceof ArrayBuffer ? new Uint8Array(input) : input;
  if (!(bytes instanceof Uint8Array) || bytes.length < 8 || bytes.length > MAX_MESSAGE ||
      bytes[0] !== 70 || bytes[1] !== 85 || bytes[2] !== VERSION || bytes[3] !== 0)
    throw Error('Invalid binary proxy frame');
  const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(4, true);
  if (header > MAX_HEADER || header > bytes.length - 8 || bytes.length - 8 - header > MAX_TRANSFER)
    throw Error('Invalid binary proxy frame length');
  const frame = JSON.parse(decoder.decode(bytes.subarray(8, 8 + header)));
  if (!frame || typeof frame !== 'object' || Array.isArray(frame)) throw Error('Invalid proxy metadata');
  const owner = payloadOwner(frame), data = bytes.subarray(8 + header);
  if ('data' in (frame.operation ?? {}) || 'data' in (frame.completion ?? {}))
    throw Error('Payload must be binary');
  if (owner) {
    if ('data' in owner) throw Error('Payload must be binary');
    owner.data = data;
  } else if (data.length) throw Error('Unexpected binary payload');
  return frame;
}
