import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stagePeerAssets } from './stage-peer-assets.ts';
await test('stages the observer next to verified wasm-bindgen peer snippets without rebuilding any live asset', async () => {
  const root = await mkdtemp(join(tmpdir(), 'peer-assets-'));
  try {
    const source = join(root, 'source'),
      pkg = join(root, 'pkg');
    const web = join(pkg, 'snippets', 'crate-hash', 'web', 'build');
    await mkdir(source);
    await mkdir(web, { recursive: true });
    const peer = "import { observeRequest } from './share-traffic.js';\nimport { VERSION } from './wire.js';";
    await writeFile(join(source, 'peer.js'), peer);
    await writeFile(join(source, 'share-traffic.js'), 'export function observeRequest() {}');
    // Reached only through wire.js: staging follows imports transitively.
    await writeFile(join(source, 'wire.js'), "export const VERSION = 4;\nexport { Pipeline } from './pipeline.js';");
    await writeFile(join(source, 'pipeline.js'), 'export class Pipeline {}');
    await writeFile(join(web, 'peer.js'), peer);
    await stagePeerAssets(pkg, source);
    assert.equal(
      await readFile(join(web, 'share-traffic.js'), 'utf8'),
      await readFile(join(source, 'share-traffic.js'), 'utf8'),
    );
    assert.equal(
      await readFile(join(web, 'wire.js'), 'utf8'),
      await readFile(join(source, 'wire.js'), 'utf8'),
    );
    assert.equal(
      await readFile(join(web, 'pipeline.js'), 'utf8'),
      await readFile(join(source, 'pipeline.js'), 'utf8'),
    );
    await writeFile(join(web, 'peer.js'), 'stale');
    await assert.rejects(stagePeerAssets(pkg, source), /differs/);
    await rm(join(pkg, 'snippets', 'crate-hash'), { recursive: true });
    await assert.rejects(stagePeerAssets(pkg, source), /no peer.js/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
