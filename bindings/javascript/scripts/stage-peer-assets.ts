// wasm-bindgen copies declared JS entry modules, but not their relative imports.
import { readdir, readFile, copyFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const sources = fileURLToPath(new URL('../../../crates/usb-proxy/web/build/', import.meta.url));
/** Every module `peer.js` reaches through relative static imports, besides itself. */
export async function peerImports(source: string) {
  const found = new Set<string>(),
    pending = ['peer.js'];
  while (pending.length) {
    const text = await readFile(join(source, pending.pop()!), 'utf8');
    for (const [, name] of text.matchAll(/(?:import|export)[^'"]*?from\s*['"]\.\/([\w.-]+\.js)['"]/g))
      if (name !== 'peer.js' && !found.has(name)) {
        found.add(name);
        pending.push(name);
      }
  }
  return [...found];
}
export async function stagePeerAssets(pkg: string, source = sources) {
  const peer = await readFile(join(source, 'peer.js'));
  const imports = await peerImports(source);
  let staged = 0;
  for (const entry of await readdir(join(pkg, 'snippets'), { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const web = join(pkg, 'snippets', entry.name, 'web', 'build');
    let generated;
    try {
      generated = await readFile(join(web, 'peer.js'));
    } catch (error) {
      if (error instanceof Error && 'code' in error && error.code === 'ENOENT') continue;
      throw error;
    }
    if (!generated.equals(peer)) throw Error('wasm-bindgen peer.js differs from its source');
    for (const name of imports)
      await copyFile(join(source, name), join(web, name));
    staged++;
  }
  if (!staged) throw Error('wasm-bindgen output contains no peer.js module');
}
