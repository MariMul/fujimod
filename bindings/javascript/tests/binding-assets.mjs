import { readdir } from 'node:fs/promises';

// Enumerate built assets rather than maintaining a second list of the generated
// import graph. Only regular runtime files from dist/ and pkg/ enter the server.
export async function bindingAssets(root = new URL('../', import.meta.url)) {
  const assets = new Map();
  async function visit(directory, prefix) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = `${prefix}/${entry.name}`;
      const file = new URL(entry.name, directory);
      if (entry.isDirectory()) await visit(new URL(`${entry.name}/`, directory), path);
      else if (entry.isFile() && /\.(js|wasm|json)$/.test(entry.name)) {
        const type = entry.name.endsWith('.wasm')
          ? 'application/wasm'
          : entry.name.endsWith('.json')
            ? 'application/json'
            : 'text/javascript';
        assets.set(path, [file, type]);
      }
    }
  }
  for (const directory of ['dist', 'pkg'])
    await visit(new URL(`${directory}/`, root), `/${directory}`);
  return assets;
}
