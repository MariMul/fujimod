/** A POSIX ustar archive built from Blob parts, so large files are never copied. */
const BLOCK = 512;
const encoder = new TextEncoder();

function field(header: Uint8Array, offset: number, length: number, text: string) {
  const bytes = encoder.encode(text);
  if (bytes.length > length) throw Error(`Archive field too long: ${text}`);
  header.set(bytes, offset);
}
function octal(header: Uint8Array, offset: number, length: number, value: number) {
  field(header, offset, length, value.toString(8).padStart(length - 1, '0'));
}

function header(name: string, size: number, mtime: number): Uint8Array<ArrayBuffer> {
  const block = new Uint8Array(BLOCK);
  let prefix = '';
  let base = name;
  if (encoder.encode(name).length > 100) {
    const split = name.lastIndexOf('/', 155);
    if (split < 0) throw Error(`Archive path too long: ${name}`);
    prefix = name.slice(0, split);
    base = name.slice(split + 1);
  }
  field(block, 0, 100, base);
  octal(block, 100, 8, 0o644);
  octal(block, 108, 8, 0);
  octal(block, 116, 8, 0);
  octal(block, 124, 12, size);
  octal(block, 136, 12, Math.floor(mtime / 1000));
  field(block, 148, 8, '        ');
  field(block, 156, 1, '0');
  field(block, 257, 6, 'ustar\0');
  field(block, 263, 2, '00');
  field(block, 345, 155, prefix);
  const sum = block.reduce((total, byte) => total + byte, 0);
  field(block, 148, 8, `${sum.toString(8).padStart(6, '0')}\0 `);
  return block;
}

export class TarArchive {
  private readonly parts: BlobPart[] = [];
  private bytes = 0;
  constructor(private readonly mtime = Date.now()) {}
  /** Append a file. `data` parts are referenced, not copied. */
  add(name: string, data: Blob | Uint8Array<ArrayBuffer> | readonly Uint8Array<ArrayBuffer>[]) {
    const blob =
      data instanceof Blob ? data : new Blob(data instanceof Uint8Array ? [data] : [...data]);
    this.parts.push(header(name, blob.size, this.mtime), blob);
    const padding = (BLOCK - (blob.size % BLOCK)) % BLOCK;
    if (padding) this.parts.push(new Uint8Array(padding));
    this.bytes += BLOCK + blob.size + padding;
  }
  get size() {
    return this.bytes + 2 * BLOCK;
  }
  blob(): Blob {
    return new Blob([...this.parts, new Uint8Array(2 * BLOCK)], { type: 'application/x-tar' });
  }
}
