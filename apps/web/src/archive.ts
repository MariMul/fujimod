/** Store-only ZIP for browser downloads; no third-party library. */
const table = Uint32Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = (c >>> 1) ^ ((c & 1) ? 0xedb88320 : 0);
  return c >>> 0;
});
export function crc32(data: Uint8Array) {
  let crc = 0xffffffff;
  for (const value of data) crc = (crc >>> 8) ^ table[(crc ^ value) & 255];
  return (crc ^ 0xffffffff) >>> 0;
}
export function zipFiles(files: [string, Uint8Array<ArrayBuffer> ][]): Blob {
  const local: Uint8Array<ArrayBuffer>[] = [], directory: Uint8Array<ArrayBuffer>[] = [];
  let offset = 0, directorySize = 0;
  for (const [name, data] of files) {
    const filename = new TextEncoder().encode(name), crc = crc32(data);
    const header = new Uint8Array(30 + filename.length), view = new DataView(header.buffer);
    view.setUint32(0, 0x04034b50, true); view.setUint16(4, 20, true);
    view.setUint16(12, 33, true); // 1980-01-01, fixed deterministic DOS date
    view.setUint32(14, crc, true); view.setUint32(18, data.length, true); view.setUint32(22, data.length, true);
    view.setUint16(26, filename.length, true); header.set(filename, 30);
    local.push(header, data);
    const central = new Uint8Array(46 + filename.length), cv = new DataView(central.buffer);
    cv.setUint32(0, 0x02014b50, true); cv.setUint16(4, 20, true); cv.setUint16(6, 20, true);
    cv.setUint16(14, 33, true); cv.setUint32(16, crc, true);
    cv.setUint32(20, data.length, true); cv.setUint32(24, data.length, true);
    cv.setUint16(28, filename.length, true); cv.setUint32(42, offset, true); central.set(filename, 46);
    directory.push(central); directorySize += central.length; offset += header.length + data.length;
  }
  const end = new Uint8Array(22), ev = new DataView(end.buffer);
  ev.setUint32(0, 0x06054b50, true); ev.setUint16(8, files.length, true); ev.setUint16(10, files.length, true);
  ev.setUint32(12, directorySize, true); ev.setUint32(16, offset, true);
  return new Blob([...local, ...directory, end], { type: 'application/zip' });
}
