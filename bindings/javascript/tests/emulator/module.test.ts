import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { EmulatorModule } from '../../dist/emulator/abi.js';
import { instantiateEmulatorModule, parseEmulatorManifest } from '../../dist/emulator/module.js';

function emulatorModule() {
  return {
    HEAPU8: new Uint8Array(64),
    _camera_alloc: () => 8,
    _camera_free: () => {},
    _camera_create: () => 1,
    _camera_session_create: () => 1,
    _camera_command: () => 0,
    _camera_scene_buffer: () => 0,
    _camera_scene_commit: () => 0,
    _camera_destroy: () => 0,
    _camera_run_until: () => 0,
    _camera_input: () => 0,
    _camera_status: () => 0,
    _camera_frame: () => 1,
    _camera_error: () => 0,
    tcgJitStats: () => ({ enabled: false }),
  } satisfies EmulatorModule;
}

test('module factory receives independent binary and asset resolution', async () => {
  let received: { wasmBinary: ArrayBuffer; locateFile(path: string): string } | undefined;
  const binary = new Uint8Array([0, 97, 115, 109]).buffer;
  const loaded = await instantiateEmulatorModule(
    async (options) => {
      received = options;
      return emulatorModule();
    },
    binary,
    (path) => new URL(path, 'https://assets.example/runtime/camera.wasm').href,
  );
  assert.equal(loaded.HEAPU8.byteLength, 64);
  assert.equal(received?.wasmBinary, binary);
  assert.equal(received?.locateFile('camera.wasm'), 'https://assets.example/runtime/camera.wasm');
});

test('module validation rejects incomplete generated modules', async () => {
  await assert.rejects(
    instantiateEmulatorModule(
      async () => ({ HEAPU8: new Uint8Array(4) }),
      new ArrayBuffer(0),
      () => '',
    ),
    /Missing C ABI export/,
  );
});

test('manifest parsing validates the versioned runtime and observation shape', () => {
  const manifest = parseEmulatorManifest({
    schema: 1,
    abi: 1,
    module: 'camera.mjs',
    wasm: 'camera.wasm',
    construction: 'session',
    executionPolicy: 'exact',
    session: {},
    ticksPerSecond: 12_000_000,
    bootTicks: 0,
    output: 1,
    observations: [{ name: 'frame_count', address: '1234', size: 4 }],
  });
  assert.equal(manifest.output, 1);
  assert.equal(manifest.executionPolicy, 'exact');
  assert.equal(parseEmulatorManifest({ ...manifest, executionPolicy: undefined }).executionPolicy, undefined);
  assert.throws(
    () => parseEmulatorManifest({ ...manifest, executionPolicy: 'invalid' }),
    /Unsupported emulator manifest/,
  );
  assert.throws(
    () =>
      parseEmulatorManifest({
        ...manifest,
        observations: [{ name: '0bad', address: 'nope', size: 0 }],
      }),
    /Invalid observation declaration/,
  );
  assert.throws(
    () => parseEmulatorManifest({ ...manifest, abi: 2 }),
    /Unsupported emulator manifest/,
  );
});
