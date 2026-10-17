import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { join, relative } from 'node:path';
import { pathToFileURL } from 'node:url';

export function validateGlb(data, name) {
  if (data.length < 20 || data.toString('ascii', 0, 4) !== 'glTF' ||
      data.readUInt32LE(4) !== 2 || data.readUInt32LE(8) !== data.length ||
      data.readUInt32LE(16) !== 0x4e4f534a || data.readUInt32LE(12) + 20 > data.length)
    throw Error(`${name}: expected real glTF 2 binary bytes; fetch Git LFS objects before building.`);
  const json = JSON.parse(data.toString('utf8', 20, 20 + data.readUInt32LE(12)));
  if (json.asset?.version !== '2.0') throw Error(`${name}: invalid glTF asset version.`);
}
async function models(directory) {
  const paths = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) paths.push(...await models(path));
    else if (entry.name.endsWith('.glb')) paths.push(path);
  }
  return paths;
}
export async function verifyModels(output, source = 'public/devices') {
  const receipts = [];
  for (const path of await models(source)) {
    const name = relative('public', path), original = await readFile(path), deployed = await readFile(join(output, name));
    validateGlb(original, path); validateGlb(deployed, join(output, name));
    const sha256 = data => createHash('sha256').update(data).digest('hex');
    if (sha256(original) !== sha256(deployed)) throw Error(`${name}: deployed model differs from source.`);
    receipts.push({ path: name, bytes: deployed.length, sha256: sha256(deployed) });
  }
  if (!receipts.length) throw Error('No model assets found.');
  return receipts;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  console.log(JSON.stringify(await verifyModels(process.argv[2] || 'dist'), null, 2));
