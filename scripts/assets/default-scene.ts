/** Materialize the single, hash-verified RAF shared by every default-scene consumer. */
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, writeFile, rm } from 'node:fs/promises';
import { dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { zstdDecompressSync } from 'node:zlib';

export const originalSha256 = '2fdbda765e9abf147d4bdfdd5a2060d2c8702d7ad53561a5bde664bc50d53cf4';
export const defaultSceneRaf = fileURLToPath(
  new URL('../../build/assets/default-scene.raf', import.meta.url),
);
const original = new URL('../../apps/web/src/engine/assets/default-scene.raf.zst', import.meta.url);
const hash = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex');

export async function prepareDefaultScene(): Promise<string> {
  try {
    if (hash(await readFile(defaultSceneRaf)) === originalSha256) return defaultSceneRaf;
  } catch (error) {
    if (!(error instanceof Error && 'code' in error && error.code === 'ENOENT')) throw error;
  }
  const bytes = zstdDecompressSync(await readFile(original));
  if (hash(bytes) !== originalSha256)
    throw Error('Default RAF does not match the recorded original SHA-256.');
  await mkdir(dirname(defaultSceneRaf), { recursive: true });
  const temporary = `${defaultSceneRaf}.${process.pid}.${randomUUID()}.tmp`;
  try {
    await writeFile(temporary, bytes);
    await rename(temporary, defaultSceneRaf);
  } finally {
    await rm(temporary, { force: true });
  }
  return defaultSceneRaf;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  console.log(await prepareDefaultScene());
