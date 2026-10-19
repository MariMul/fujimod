interface BrowserManifest {
  schema: number;
  files: { path: string; asset: string; sha256: string; bytes: number }[];
  catalog: unknown;
}
import { createHash } from 'node:crypto';
import { mkdir, writeFile, rm } from 'node:fs/promises';
import { dirname } from 'node:path';
import { validateCatalog, type Catalog } from '../src/catalog.ts';

export function requireBrowserRelease(assets: { name: string }[]) {
  if (!assets.some((asset) => asset.name === 'browser.json'))
    throw Error(
      'The release has no browser runtime inventory (browser.json). Publish emulator artifacts before deploying the website.',
    );
}

/** Inventory-only distribution: no extraction of unchecked archive paths. */
export async function stageBrowser(
  manifest: BrowserManifest,
  download: (asset: string) => Promise<Uint8Array>,
) {
  if (
    manifest?.schema !== 1 ||
    !Array.isArray(manifest.files) ||
    !manifest.files.length ||
    !manifest.catalog
  )
    throw Error('Invalid browser manifest.');
  const seen = new Set();
  for (const entry of manifest.files) {
    if (
      !/^runtime\/[a-zA-Z0-9_.\/-]+$/.test(entry.path) ||
      entry.path.split('/').includes('..') ||
      !/^[a-zA-Z0-9_.-]+$/.test(entry.asset) ||
      !/^[a-f0-9]{64}$/.test(entry.sha256) ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes <= 0 ||
      seen.has(entry.path) ||
      /\.(dat|rom)$/i.test(entry.path)
    )
      throw Error('Invalid browser artifact entry.');
    seen.add(entry.path);
  }
  const catalog = validateCatalog(manifest.catalog as Catalog);
  for (const firmware of catalog.firmwares) {
    const config = firmware.emulate;
    if (!config) continue;
    for (const path of [
      config.inputs,
      ...[
        'sources.zip',
        '_native.so',
        'firmware-inputs.zip',
        'pyodide/pyodide.mjs',
        'pyodide/pyodide.asm.mjs',
        'pyodide/pyodide.asm.wasm',
        'pyodide/pyodide-lock.json',
        'pyodide/python_stdlib.zip',
      ].map((name) => config.bundle + name),
    ])
      if (!seen.has(path)) throw Error(`Missing installed emulator artifact: ${path}`);
  }
  await rm('public/runtime', { recursive: true, force: true });
  for (const entry of manifest.files) {
    const data = await download(entry.asset);
    if (
      data.length !== entry.bytes ||
      createHash('sha256').update(data).digest('hex') !== entry.sha256
    )
      throw Error(`Browser artifact integrity failure: ${entry.path}`);
    const path = `public/${entry.path}`;
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, data);
  }
  await writeFile('public/runtime/catalog.json', JSON.stringify(manifest.catalog));
}
