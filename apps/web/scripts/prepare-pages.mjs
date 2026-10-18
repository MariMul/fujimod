/** Stage verified released artifacts for Vite. Source comes only from this repo. */
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { validateIndex, verifyPatch } from '../src/patch/catalog.ts';
const repo = process.env.GITHUB_REPOSITORY || 'justjake/fujimod';
const gh = (args) => execFileSync('gh', args, { encoding: 'utf8' }).trim();
const tag =
  process.env.RELEASE_TAG ||
  gh(['api', `repos/${repo}/releases`, '--jq', '[.[] | select(.draft == false)][0].tag_name']);
if (!tag || tag === 'null') throw new Error('No published patch release.');
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
// Optional browser artifacts use an inventory of hashes and a root directory,
// never firmware or boot ROM. Existing patch-only releases still work.
const assets = JSON.parse(gh(['release', 'view', tag, '--repo', repo, '--json', 'assets'])).assets;
if (assets.some((asset) => asset.name === 'browser.json')) {
  gh(['release', 'download', tag, '--repo', repo, '--pattern', 'browser.json', '--dir', dest]);
  const { stageBrowser } = await import('./stage-browser.mjs');
  await stageBrowser(JSON.parse(await readFile(`${dest}/browser.json`, 'utf8')), async (name) => {
    gh(['release', 'download', tag, '--repo', repo, '--pattern', name, '--dir', dest]);
    return new Uint8Array(await readFile(`${dest}/${name}`));
  });
} else {
  // The catalog describes installed capabilities, including patch-only releases.
  const catalog = JSON.parse(await readFile('public/catalog.json', 'utf8'));
  for (const entry of catalog.firmwares) delete entry.emulate;
  await mkdir('public/runtime', { recursive: true });
  await writeFile('public/runtime/catalog.json', JSON.stringify(catalog));
}
console.log(`Prepared ${tag}: ${index.firmwares.length} supported patch(es).`);
