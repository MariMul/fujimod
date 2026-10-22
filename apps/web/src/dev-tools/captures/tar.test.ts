import { expect, test } from 'vite-plus/test';
import { TarArchive } from './tar';

async function entries(blob: Blob) {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  const text = (from: number, length: number) =>
    new TextDecoder().decode(bytes.subarray(from, from + length)).replace(/\0.*$/s, '');
  const found: { name: string; data: Uint8Array }[] = [];
  for (let at = 0; at + 512 <= bytes.length;) {
    if (bytes.subarray(at, at + 512).every((byte) => byte === 0)) break;
    const sum = bytes
      .subarray(at, at + 512)
      .reduce((total, byte, index) => total + (index >= 148 && index < 156 ? 32 : byte), 0);
    expect(parseInt(text(at + 148, 8), 8)).toBe(sum);
    const size = parseInt(text(at + 124, 12), 8);
    const prefix = text(at + 345, 155);
    const name = text(at, 100);
    found.push({
      name: prefix ? `${prefix}/${name}` : name,
      data: bytes.slice(at + 512, at + 512 + size),
    });
    at += 512 + Math.ceil(size / 512) * 512;
  }
  return found;
}

test('ustar entries keep names, checksums, padding and bytes', async () => {
  const tar = new TarArchive(0);
  const long = `states/${'a'.repeat(90)}/firmware-0x880000.bin`;
  tar.add('manifest.json', new TextEncoder().encode('{}'));
  tar.add(long, [Uint8Array.of(1, 2), Uint8Array.of(3)]);
  const blob = tar.blob();
  expect(blob.size % 512).toBe(0);
  expect(blob.size).toBe(tar.size);
  const files = await entries(blob);
  expect(files.map((file) => file.name)).toEqual(['manifest.json', long]);
  expect([...files[1].data]).toEqual([1, 2, 3]);
});
