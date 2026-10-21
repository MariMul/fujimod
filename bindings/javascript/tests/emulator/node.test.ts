import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { loadNodeModule } from '../../dist/emulator/node.mjs';

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('Node loader resolves a module file and its adjacent Wasm binary', async () => {
  const target = resolve(packageRoot, '../../target');
  await mkdir(target, { recursive: true });
  const directory = await mkdtemp(join(target, 'javascript-emulator-node-'));
  try {
    const modulePath = join(directory, 'camera.mjs');
    const wasmPath = join(directory, 'camera.wasm');
    await writeFile(wasmPath, new Uint8Array([0, 97, 115, 109]));
    await writeFile(
      modulePath,
      `export default async ({ wasmBinary }) => {
        if (wasmBinary.byteLength !== 4) throw Error('adjacent binary was not loaded');
        return {
          HEAPU8: new Uint8Array(64),
          _camera_alloc: () => 8, _camera_free: () => {}, _camera_create: () => 1,
          _camera_session_create: () => 1, _camera_command: () => 0,
          _camera_scene_buffer: () => 0, _camera_scene_commit: () => 0,
          _camera_destroy: () => 0, _camera_run_until: () => 0, _camera_input: () => 0,
          _camera_status: () => 0, _camera_frame: () => 1, _camera_error: () => 0,
          tcgJitStats: () => ({ enabled: false }),
        };
      };\n`,
    );
    const loaded = await loadNodeModule(modulePath);
    assert.equal(loaded.HEAPU8.byteLength, 64);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('browser entry remains independent of Node builtins', async () => {
  const entry = new URL('../../dist/emulator/index.js', import.meta.url);
  const source = await readFile(entry, 'utf8');
  assert.doesNotMatch(source, /from ['"]node:/);
  assert.doesNotMatch(source, /import\(['"]node:/);
});
