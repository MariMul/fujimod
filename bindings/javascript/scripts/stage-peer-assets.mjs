// wasm-bindgen copies declared JS entry modules, but not their relative imports.
import { readdir, readFile, copyFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const sources = fileURLToPath(new URL('../../../crates/usb-proxy/web/', import.meta.url));
export async function stagePeerAssets(pkg, source = sources) {
  const peer = await readFile(join(source, 'peer.js'));
  let staged = 0;
  for (const entry of await readdir(join(pkg, 'snippets'), { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const web = join(pkg, 'snippets', entry.name, 'web');
    let generated;
    try { generated = await readFile(join(web, 'peer.js')); }
    catch (error) { if (error.code === 'ENOENT') continue; throw error; }
    if (!generated.equals(peer)) throw Error('wasm-bindgen peer.js differs from its source');
    for (const name of ['share-traffic.js', 'wire.js'])
      await copyFile(join(source, name), join(web, name));
    staged++;
  }
  if (!staged) throw Error('wasm-bindgen output contains no peer.js module');
}
