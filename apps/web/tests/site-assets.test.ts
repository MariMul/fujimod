import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stageSite } from '../scripts/stage-site.ts';
import type { Catalog } from '../src/catalog';

test('a failed local asset preparation preserves the installed website assets', async () => {
  const root = await mkdtemp(join(tmpdir(), 'site-assets-'));
  const runtime = join(root, 'input'),
    destination = join(root, 'public');
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
          bundle: 'runtime/test-100/',
          sceneInput: false,
        },
      },
    ],
  };
  try {
    await mkdir(join(destination, 'runtime'), { recursive: true });
    await writeFile(join(destination, 'runtime/catalog.json'), 'installed');
    await mkdir(join(runtime, 'test-100'), { recursive: true });
    const inputs = { runtime, media: join(root, 'media'), patches: join(root, 'patches') };
    await writeFile(join(runtime, 'catalog.json'), JSON.stringify({ schema: 1, firmwares: [] }));
    await assert.rejects(stageSite(inputs, destination), /requires runnable emulators/);
    await writeFile(join(runtime, 'catalog.json'), JSON.stringify(catalog));
    await assert.rejects(stageSite(inputs, destination), /qualification.json/);
    await writeFile(
      join(runtime, 'test-100/qualification.json'),
      JSON.stringify({
        firmware_sha256: '1'.repeat(64),
        execution_mode: 'strict',
        counts_as_boot: true,
        menu: true,
        failure: null,
      }),
    );
    await writeFile(
      join(runtime, 'test-100/browser-qualification.json'),
      JSON.stringify({ sha256: '2'.repeat(64), passed: true }),
    );
    await assert.rejects(stageSite(inputs, destination), /qualification failed/);
    await writeFile(
      join(runtime, 'test-100/browser-qualification.json'),
      JSON.stringify({ sha256: '1'.repeat(64), passed: true }),
    );
    await assert.rejects(stageSite(inputs, destination), /camera.wasm/);
    for (const name of ['camera.wasm', 'camera.mjs', 'wasm.json', 'runtime-NOTICES.txt'])
      await writeFile(join(runtime, 'test-100', name), 'test');
    await assert.rejects(stageSite(inputs, destination), /patches.json/);
    assert.equal(await readFile(join(destination, 'runtime/catalog.json'), 'utf8'), 'installed');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
