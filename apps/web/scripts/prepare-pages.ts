/** Stage verified released artifacts for Vite. Source comes only from this repo. */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { validateIndex, verifyPatch } from '../src/patch/catalog.ts';
import { stageBrowser, requireBrowserRelease } from './stage-browser.ts';
const repo = process.env.GITHUB_REPOSITORY || 'justjake/fujimod';
const gh = (args: string[]) => execFileSync('gh', args, { encoding: 'utf8' }).trim();
const tag =
  process.env.RELEASE_TAG ||
  gh(['api', `repos/${repo}/releases`, '--jq', '[.[] | select(.draft == false)][0].tag_name']);
if (!tag || tag === 'null') throw new Error('No published patch release.');
const assets: { name: string }[] = JSON.parse(
  gh(['release', 'view', tag, '--repo', repo, '--json', 'assets']),
).assets;
requireBrowserRelease(assets);
const dest = 'public/artifacts';
await rm(dest, { recursive: true, force: true });
await mkdir(dest, { recursive: true });
gh(['release', 'download', tag, '--repo', repo, '--pattern', 'patches.json', '--dir', dest]);
const index = validateIndex(JSON.parse(await readFile(`${dest}/patches.json`, 'utf8')));
if (index.release !== tag) throw new Error('Release tag and index differ.');
for (const entry of index.firmwares) {
  const name = index.patches[entry.sha256];
  gh(['release', 'download', tag, '--repo', repo, '--pattern', name, '--dir', dest]);
  await verifyPatch(index, entry, new Uint8Array(await readFile(`${dest}/${name}`)));
}
// The website requires runnable emulation artifacts in every published build.
gh(['release', 'download', tag, '--repo', repo, '--pattern', 'browser.json', '--dir', dest]);
await stageBrowser(JSON.parse(await readFile(`${dest}/browser.json`, 'utf8')), async (name) => {
  gh(['release', 'download', tag, '--repo', repo, '--pattern', name, '--dir', dest]);
  return new Uint8Array(await readFile(`${dest}/${name}`));
});
// Optional features-catalog media: preview clips and stills, verified by hash.
if (assets.some((asset) => asset.name === 'docs-media.json')) {
  gh(['release', 'download', tag, '--repo', repo, '--pattern', 'docs-media.json', '--dir', dest]);
  const media: { files: { path: string; asset: string; sha256: string }[] } = JSON.parse(
    await readFile(`${dest}/docs-media.json`, 'utf8'),
  );
  await mkdir(`${dest}/docs`, { recursive: true });
  for (const file of media.files) {
    if (!/^[0-9a-f]{24}\.(mp4|webp)$/.test(file.path))
      throw new Error(`Invalid media ${file.path}`);
    gh(['release', 'download', tag, '--repo', repo, '--pattern', file.asset, '--dir', dest]);
    const data = await readFile(`${dest}/${file.asset}`);
    if (createHash('sha256').update(data).digest('hex') !== file.sha256)
      throw new Error(`Media hash differs: ${file.path}`);
    await writeFile(`${dest}/docs/${file.path}`, data);
    await rm(`${dest}/${file.asset}`);
  }
}
console.log(`Prepared ${tag}: ${index.firmwares.length} supported patch(es).`);
