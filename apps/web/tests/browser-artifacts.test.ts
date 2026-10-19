import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stageBrowser } from '../scripts/stage-browser.ts';
test('released runtime staging rejects traversal, firmware files and corrupted downloads', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'fujimod-artifacts-')),
    previous = process.cwd();
  process.chdir(directory);
  const data = new TextEncoder().encode('synthetic runtime');
  const entry = {
    path: 'runtime/test/engine.wasm',
    asset: 'test--engine.wasm',
    bytes: data.length,
    sha256: createHash('sha256').update(data).digest('hex'),
  };
  const manifest = { schema: 1, files: [entry], catalog: { schema: 1, firmwares: [] } };
  try {
    for (const path of [
      'runtime/../escape',
      '/runtime/escape',
      'runtime/test/FPUPDATE.DAT',
      'runtime/test/boot.rom',
    ])
      await assert.rejects(
        stageBrowser({ ...manifest, files: [{ ...entry, path }] }, async () => data),
      );
    await assert.rejects(
      stageBrowser(manifest, async () => new Uint8Array([1])),
      /integrity/,
    );
    await stageBrowser(manifest, async () => data);
    assert.deepEqual(new Uint8Array(await readFile('public/runtime/test/engine.wasm')), data);
    assert.deepEqual(
      JSON.parse(await readFile('public/runtime/catalog.json', 'utf8')),
      manifest.catalog,
    );
  } finally {
    process.chdir(previous);
    await rm(directory, { recursive: true, force: true });
  }
});
