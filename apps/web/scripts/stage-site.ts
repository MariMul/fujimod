/** Stage generated website assets locally before their direct Cloudflare upload. */
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { validateCatalog, type Catalog } from '../src/catalog.ts';
import { validateIndex, verifyPatch } from '../src/patch/catalog.ts';

export interface SiteInputs {
  runtime: string;
  patches: string;
  media: string;
}
export async function stageSite(inputs: SiteInputs, destination = 'public') {
  const catalog = validateCatalog(
    JSON.parse(await readFile(join(inputs.runtime, 'catalog.json'), 'utf8')) as Catalog,
  );
  const emulators = catalog.firmwares.filter((entry) => entry.emulate);
  if (!emulators.length) throw Error('Website deployment requires runnable emulators.');
  const target = resolve(destination);
  await mkdir(dirname(target), { recursive: true });
  const temporary = await mkdtemp(join(dirname(target), '.site-assets-'));
  const files: { path: string; bytes: number; sha256: string }[] = [];
  async function copy(source: string, path: string) {
    const data = await readFile(source);
    if (!data.length || data.length > 25 * 1024 * 1024) throw Error(`Invalid asset size: ${path}`);
    const staged = join(temporary, path);
    await mkdir(dirname(staged), { recursive: true });
    await writeFile(staged, data);
    files.push({
      path,
      bytes: data.length,
      sha256: createHash('sha256').update(data).digest('hex'),
    });
  }
  try {
    for (const firmware of emulators) {
      const config = firmware.emulate!;
      if (
        config.bundle !== `runtime/${firmware.id}/` ||
        config.inputs !== config.bundle + 'inputs.json'
      )
        throw Error(`Unexpected runtime location: ${firmware.id}`);
      // USB uses a separate artifact inventory and is enabled only when bundled.
      delete config.usb;
      const directory = join(inputs.runtime, firmware.id);
      const native = JSON.parse(await readFile(join(directory, 'qualification.json'), 'utf8')) as {
        firmware_sha256?: string;
        execution_mode?: string;
        counts_as_boot?: boolean;
        menu?: boolean;
        failure?: unknown;
      };
      const browser = JSON.parse(
        await readFile(join(directory, 'browser-qualification.json'), 'utf8'),
      ) as {
        sha256?: string;
        passed?: boolean;
      };
      if (
        native.firmware_sha256 !== firmware.sha256 ||
        native.execution_mode !== 'strict' ||
        !native.counts_as_boot ||
        !native.menu ||
        native.failure ||
        browser.sha256 !== firmware.sha256 ||
        browser.passed !== true
      )
        throw Error(`Emulator qualification failed: ${firmware.id}`);
      for (const name of [
        '_native.so',
        'sources.zip',
        'firmware-inputs.zip',
        'inputs.json',
        'runtime-NOTICES.txt',
      ])
        await copy(join(directory, name), config.bundle + name);
      const pyodide = join(directory, 'pyodide');
      const lock = JSON.parse(await readFile(join(pyodide, 'pyodide-lock.json'), 'utf8')) as {
        packages: Record<string, { file_name: string; depends: string[] }>;
      };
      const packages = new Set<string>();
      function requirePackage(name: string) {
        const item = lock.packages[name];
        if (!item || !/^[a-zA-Z0-9_.+-]+$/.test(item.file_name))
          throw Error(`Invalid Pyodide package: ${name}`);
        if (packages.has(item.file_name)) return;
        packages.add(item.file_name);
        item.depends.forEach(requirePackage);
      }
      ['numpy', 'pillow'].forEach(requirePackage);
      for (const name of [
        'pyodide.mjs',
        'pyodide.asm.mjs',
        'pyodide.asm.wasm',
        'pyodide-lock.json',
        'python_stdlib.zip',
        ...packages,
      ])
        await copy(join(pyodide, name), config.bundle + 'pyodide/' + name);
    }
    const catalogBytes = Buffer.from(JSON.stringify(catalog));
    await writeFile(join(temporary, 'runtime/catalog.json'), catalogBytes);
    files.push({
      path: 'runtime/catalog.json',
      bytes: catalogBytes.length,
      sha256: createHash('sha256').update(catalogBytes).digest('hex'),
    });
    const index = validateIndex(
      JSON.parse(await readFile(join(inputs.patches, 'patches.json'), 'utf8')),
    );
    await copy(join(inputs.patches, 'patches.json'), 'artifacts/patches.json');
    for (const firmware of index.firmwares) {
      const name = index.patches[firmware.sha256];
      await verifyPatch(
        index,
        firmware,
        new Uint8Array(await readFile(join(inputs.patches, name))),
      );
      await copy(join(inputs.patches, name), 'artifacts/' + name);
    }
    for (const name of await readdir(inputs.media)) {
      if (!/^[0-9a-f]{24}\.(mp4|webp)$/.test(name))
        throw Error(`Unexpected feature media: ${name}`);
      await copy(join(inputs.media, name), 'artifacts/docs/' + name);
    }
    if (!files.some((file) => file.path.startsWith('artifacts/docs/')))
      throw Error('Feature media is missing.');
    await mkdir(target, { recursive: true });
    for (const directory of ['runtime', 'artifacts']) {
      await rm(join(target, directory), { recursive: true, force: true });
      await rename(join(temporary, directory), join(target, directory));
    }
    await writeFile(
      join(target, 'site-assets.json'),
      JSON.stringify({ schema: 1, files }, null, 2) + '\n',
    );
    return { emulators: emulators.length, files: files.length };
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
