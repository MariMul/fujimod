import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import test from 'node:test';
import { bindingAssets } from '../../bindings/javascript/tests/binding-assets.mjs';

await test('browser servers include nested generated imports and restrict their asset surface', async (t) => {
  const output = resolve('target/ci');
  await mkdir(output, { recursive: true });
  const directory = await mkdtemp(`${output}/binding-assets-`);
  t.after(() => rm(directory, { recursive: true, force: true }));
  await mkdir(`${directory}/dist/nested`, { recursive: true });
  await mkdir(`${directory}/pkg`, { recursive: true });
  for (const name of [
    'dist/index.js',
    'dist/nested/values.js',
    'pkg/service-values.js',
    'pkg/fujimod_bg.wasm',
    'pkg/metadata.json',
    'pkg/private.d.ts',
    'pkg/source.rs',
  ]) {
    await writeFile(`${directory}/${name}`, 'fixture');
  }
  await symlink(`${directory}/pkg/source.rs`, `${directory}/dist/escape.js`);
  const assets = await bindingAssets(pathToFileURL(`${directory}/`));
  assert.deepEqual(
    [...assets.keys()].sort((a, b) => a.localeCompare(b)),
    [
      '/dist/index.js',
      '/dist/nested/values.js',
      '/pkg/fujimod_bg.wasm',
      '/pkg/metadata.json',
      '/pkg/service-values.js',
    ],
  );
  assert.equal(assets.get('/pkg/fujimod_bg.wasm')[1], 'application/wasm');
  assert.equal(assets.get('/pkg/metadata.json')[1], 'application/json');
  assert.equal(
    assets.get('/dist/nested/values.js')[0].href,
    pathToFileURL(`${directory}/dist/nested/values.js`).href,
  );
});
