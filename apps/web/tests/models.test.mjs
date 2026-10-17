import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { validateGlb } from '../scripts/verify-models.mjs';
test('deployment rejects LFS pointers, truncated GLBs and wrong declared lengths', () => {
  for (const value of [Buffer.from('version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 123\n'), Buffer.alloc(19)])
    assert.throws(() => validateGlb(value, 'test.glb'), /real glTF/);
  const json = Buffer.from('{"asset":{"version":"2.0"}}  ');
  const data = Buffer.alloc(20 + json.length);
  data.write('glTF'); data.writeUInt32LE(2, 4); data.writeUInt32LE(data.length, 8);
  data.writeUInt32LE(json.length, 12); data.writeUInt32LE(0x4e4f534a, 16); json.copy(data, 20);
  assert.doesNotThrow(() => validateGlb(data, 'test.glb'));
  data.writeUInt32LE(data.length + 4, 8);
  assert.throws(() => validateGlb(data, 'test.glb'), /real glTF/);
});
