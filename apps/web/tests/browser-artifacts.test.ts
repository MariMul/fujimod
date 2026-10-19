import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stageBrowser, requireBrowserRelease } from '../scripts/stage-browser.ts';
import type { Catalog } from '../src/catalog';
test('website publication refuses a release without emulator runtime assets', () => {
  assert.throws(
    () => requireBrowserRelease([{ name: 'patches.json' }]),
    /no browser runtime inventory/,
  );
  assert.doesNotThrow(() =>
    requireBrowserRelease([{ name: 'patches.json' }, { name: 'browser.json' }]),
  );
});
test('an advertised emulator requires every runtime artifact in the release inventory', async () => {
  const catalog: Catalog = {
    schema: 1,
    firmwares: [
      {
        id: 'test-100',
        body: 'test',
        model: 'Test',
        version: '1.00',
        sha256: '1'.repeat(64),
        bytes: 42,
        release_date: '',
        download_url: 'https://example.org/firmware',
        emulate: {
          adapter: 'pyodide',
          bundle: 'runtime/test-100/',
          inputs: 'runtime/test-100/inputs.json',
          boot: {
            datPath: '/firmware.dat',
            prepare: { module: 'loader', function: 'prepare', options: {} },
            module: 'boot',
            sessionModule: 'session',
            limit: 100,
            step: 10,
            readyLimit: 10,
            readyWhen: ['ready'],
            sceneInput: false,
          },
        },
      },
    ],
  };
  await assert.rejects(
    stageBrowser(
      {
        schema: 1,
        catalog,
        files: [
          { path: 'runtime/test-100/unused', asset: 'unused', sha256: '0'.repeat(64), bytes: 1 },
        ],
      },
      async () => new Uint8Array([0]),
    ),
    /Missing installed emulator artifact/,
  );
});
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
