import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import { initialize, recovery } from '../dist/index.js';

await initialize({
  module_or_path: await readFile(new URL('../pkg/fujimod_bg.wasm', import.meta.url)),
});

test('recovery availability does not require an emulator board', () => {
  const status = recovery.availability('xt30-201');
  try {
    assert.equal(status.firmwareId, 'xt30-201');
    assert.equal(status.generation, 'xp4');
    assert.equal(status.canBuild, true);
    assert.deepEqual(status.missing, []);
    assert.ok(status.outputFilenames.includes('ffinit.bin'));
    assert.ok(status.outputFilenames.includes('ffboot.bin'));
  } finally {
    status.free();
  }
});

test('recovery exposes the documented power-on combinations', () => {
  const status = recovery.availability('xs10-311');
  try {
    assert.deepEqual(status.sdBoot, {
      usb: false,
      hold: [{ id: 'quick_menu_button' }, { id: 'movie_recording_button' }],
    });
    assert.deepEqual(status.firmwareUpdate, { hold: [{ id: 'display_back_button' }] });
  } finally {
    status.free();
  }
});

test('missing generation facts and incorrect DAT bytes are refused', () => {
  const status = recovery.availability('unknown-update');
  try {
    assert.equal(status.canBuild, false);
    assert.ok(status.missing.length > 0);
    assert.equal(status.sdBoot, null);
    assert.equal(status.firmwareUpdate, null);
  } finally {
    status.free();
  }
  assert.throws(() => recovery.build('xs10-311', new Uint8Array(4)), /size differs/);
  assert.throws(() => recovery.build('x100vi-131', new Uint8Array(4)), /Missing recovery facts/);
});
