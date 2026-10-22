import { test, vi } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { gzipSync } from 'node:zlib';
import { loadPreparedBundle } from './prepared-bundle';

const manifest = {
  document: 'document.json',
  bootRom: 'boot.bin',
  nand: 'nand.gz',
  settings: 'settings.bin',
};
const base = new URL('https://example.invalid/runtime/wasm.json');
test('prepared bundle inflates NAND and preserves boot ROM and MCU bytes', async () => {
  const nand = Uint8Array.from([255, 0, 1, 2, 255]);
  const files = new Map<string, string | Uint8Array>([
    ['document.json', '{"schema":1}'],
    ['boot.bin', Uint8Array.of(7, 8)],
    ['nand.gz', gzipSync(nand)],
    ['settings.bin', Uint8Array.of(9, 10)],
  ]);
  const urls: string[] = [];
  vi.stubGlobal('fetch', async (url: URL) => {
    urls.push(url.href);
    const body = files.get(url.pathname.split('/').at(-1)!);
    assert.ok(body !== undefined);
    return new Response(typeof body === 'string' ? body : new Uint8Array(body));
  });
  try {
    const loaded = await loadPreparedBundle(manifest, base);
    assert.equal(loaded.text, '{"schema":1}');
    assert.deepEqual(loaded.inputs.bootRom, Uint8Array.of(7, 8));
    assert.deepEqual(loaded.inputs.nand, nand);
    assert.deepEqual(loaded.inputs.settings, Uint8Array.of(9, 10));
    assert.equal(urls.length, 4);
    assert.ok(urls.every((url) => url.startsWith('https://example.invalid/runtime/')));
  } finally {
    vi.unstubAllGlobals();
  }
});
test('prepared bundle refuses missing media declarations and failed downloads', async () => {
  await assert.rejects(loadPreparedBundle({ document: 'document.json' }, base), /bootRom input/);
  vi.stubGlobal('fetch', async () => new Response(null, { status: 404 }));
  try {
    await assert.rejects(loadPreparedBundle(manifest, base), /input is unavailable/);
  } finally {
    vi.unstubAllGlobals();
  }
});
test('prepared bundle refuses corrupt compressed NAND', async () => {
  vi.stubGlobal('fetch', async () => new Response(Uint8Array.of(1, 2, 3)));
  try {
    await assert.rejects(loadPreparedBundle(manifest, base));
  } finally {
    vi.unstubAllGlobals();
  }
});
