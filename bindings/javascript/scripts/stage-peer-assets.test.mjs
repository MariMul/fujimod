import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stagePeerAssets } from './stage-peer-assets.mjs';
test('stages the observer next to verified wasm-bindgen peer snippets without rebuilding any live asset', async () => {
  const root = await mkdtemp(join(tmpdir(), 'peer-assets-'));
  try {
    const source = join(root, 'source'), pkg = join(root, 'pkg');
    const web = join(pkg, 'snippets', 'crate-hash', 'web');
    await mkdir(source); await mkdir(web, { recursive: true });
    const peer = "import { observeRequest } from './share-traffic.js';";
    await writeFile(join(source, 'peer.js'), peer);
    await writeFile(join(source, 'share-traffic.js'), 'export function observeRequest() {}');
    await writeFile(join(web, 'peer.js'), peer);
    await stagePeerAssets(pkg, source);
    assert.equal(await readFile(join(web, 'share-traffic.js'), 'utf8'), await readFile(join(source, 'share-traffic.js'), 'utf8'));
    await writeFile(join(web, 'peer.js'), 'stale');
    await assert.rejects(stagePeerAssets(pkg, source), /differs/);
    await rm(join(pkg, 'snippets', 'crate-hash'), { recursive: true });
    await assert.rejects(stagePeerAssets(pkg, source), /no peer.js/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
