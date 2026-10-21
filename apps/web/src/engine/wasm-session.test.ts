import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { EmulatorSession } from 'fujimod/emulator';
import { testCameraModule } from './wasm-test-module';

test('Wasm descriptors reacquire grown memory; frames borrow the heap without a pixel copy', () => {
  const fake = testCameraModule(),
    session = new EmulatorSession(fake.module);
  session.create({ schema: 1 }, new Uint8Array([1, 2]), 'resolved');
  assert.equal(fake.allocations.size, 1, 'only the persistent scratch allocation survives create');
  assert.equal(session.frame(0), undefined);
  fake.frame();
  const first = session.frame(0)!;
  assert.equal(first.rgba.buffer, fake.memory.buffer);
  assert.deepEqual([...first.rgba], [10, 20, 30, 255]);
  fake.memory.grow(1);
  session.run(1n << 54n);
  assert.equal(session.status().ticks, 1n << 54n);
  assert.equal(session.frame(0)!.rgba.buffer, fake.memory.buffer);
  assert.equal(first.rgba.byteLength, 0, 'old unshared memory views detach after growth');
  session.input('MENU', true);
  assert.equal(fake.calls.at(-1)!.options.at, 1n << 54n);
  session.dispose();
  assert.equal(fake.allocations.size, 0);
});
test('uploads are freed on creation and command errors; missing extensions refuse', () => {
  const fake = testCameraModule(),
    session = new EmulatorSession(fake.module);
  fake.createError('SHA-256 mismatch');
  assert.throws(() => session.create({}, new Uint8Array([1]), 'resolved'), /SHA-256/);
  assert.equal(fake.allocations.size, 1);
  fake.createError('');
  session.create({}, new Uint8Array(), 'resolved');
  fake.commandError('busy revision');
  assert.throws(
    () => session.command('card_read', { path: '/a', revision: 2 }, new Uint8Array([7])),
    /busy revision/,
  );
  assert.equal(fake.allocations.size, 1);
  fake.module._camera_command = undefined;
  assert.throws(() => session.command('card_state', {}), /camera_command/);
  fake.module._camera_scene_buffer = undefined;
  assert.throws(() => session.scene(null), /camera_scene_buffer/);
  session.dispose();
});
test('scene RGB writes directly into the reserved Wasm buffer and command bytes escape borrowed lifetimes', () => {
  const fake = testCameraModule(),
    session = new EmulatorSession(fake.module);
  session.create({}, new Uint8Array(), 'session');
  session.scene({ width: 1, height: 1, rgb: new Uint8Array([1, 2, 3]) });
  assert.deepEqual(fake.calls.at(-1)!.bytes, [1, 2, 3]);
  assert.throws(() => session.scene({ width: -1, height: 1, rgb: new Uint8Array() }), /dimensions/);
  const read = session.command('card_read', { path: '/DCIM/a.jpg', maxBytes: 10, revision: 1 });
  assert.deepEqual([...read.bytes], [4, 5, 6]);
  assert.notEqual(read.bytes.buffer, fake.memory.buffer);
  fake.module.HEAPU8.fill(0, 80000, 80003);
  assert.deepEqual([...read.bytes], [4, 5, 6]);
  session.scene(null);
  assert.equal(fake.calls.at(-1)!.options.enabled, 0);
  session.dispose();
});
