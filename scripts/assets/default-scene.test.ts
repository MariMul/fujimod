import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { prepareDefaultScene, originalSha256 } from './default-scene.ts';

test('the shared RAF is the recorded original and repeated preparation preserves it', async () => {
  const path = await prepareDefaultScene();
  assert.equal(
    createHash('sha256')
      .update(await readFile(path))
      .digest('hex'),
    originalSha256,
  );
  const before = await stat(path);
  assert.equal(await prepareDefaultScene(), path);
  assert.equal((await stat(path)).mtimeMs, before.mtimeMs);
});
