import { createReadStream } from 'node:fs';
import { readdir, readFile, stat } from 'node:fs/promises';
import { resolve, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createGzip } from 'node:zlib';

const root = resolve(
  fileURLToPath(new URL('../../../crates/simulation/environments/', import.meta.url)),
);
const wasmRoot = fileURLToPath(
  new URL(
    '../../../bindings/javascript/pkg/simulation/',
    import.meta.url,
  ),
);
async function assets(includeBuild = false) {
  const files = [];
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.name === 'build' && !includeBuild) continue;
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) await visit(path);
      else if (
        /\.(glb|gltf|bin|png|jpe?g|ktx2)$/.test(entry.name) ||
        entry.name === 'environment.json'
      )
        files.push(path);
    }
  }
  await visit(root);
  return files;
}
async function streamAsset(req, res, path, contentType) {
  const info = await stat(path);
  const gzip = /\.(wasm|glb)$/.test(path) && /\bgzip\b/.test(req.headers['accept-encoding'] || '');
  const etag = `"${info.size}-${info.mtimeMs}${gzip ? '-gzip' : ''}"`;
  res.setHeader('Content-Type', contentType);
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Vary', 'Accept-Encoding');
  res.setHeader('ETag', etag);
  if (req.headers['if-none-match'] === etag) {
    res.statusCode = 304;
    res.end();
    return;
  }
  if (gzip) res.setHeader('Content-Encoding', 'gzip');
  else res.setHeader('Content-Length', info.size);
  if (req.method === 'HEAD') { res.end(); return; }
  const stream = createReadStream(path);
  (gzip ? stream.pipe(createGzip({ level: 1 })) : stream).pipe(res);
}
/** Serve and package the crate's environment assets without a second source tree. */
export function simulationAssets() {
  const serve = (server) => {
    server.middlewares.use(async (req, res, next) => {
      const pathname = new URL(req.url ?? '/', 'http://localhost').pathname;
      const backend = ['webgl2', 'webgpu'].find((backend) =>
        pathname === `/@fs${resolve(wasmRoot, backend, 'fujimod_simulation_bg.wasm')}`);
      if (backend) {
        const wasmPath = resolve(wasmRoot, backend, 'fujimod_simulation_bg.wasm');
        await streamAsset(req, res, wasmPath, 'application/wasm');
        return;
      }
      if (!pathname.startsWith('/environments/')) return next();
      let path;
      try {
        path = resolve(root, decodeURIComponent(pathname.slice('/environments/'.length)));
      } catch {
        res.statusCode = 400;
        res.end();
        return;
      }
      if (!path.startsWith(root + sep) || !(await assets(true)).includes(path)) {
        res.statusCode = 404;
        res.end();
        return;
      }
      await streamAsset(req, res, path, path.endsWith('.json') ? 'application/json'
        : path.endsWith('.glb') ? 'model/gltf-binary' : 'application/octet-stream');
    });
  };
  return {
    name: 'simulation-assets',
    configureServer: serve,
    configurePreviewServer: serve,
    async generateBundle() {
      for (const path of await assets())
        this.emitFile({
          type: 'asset',
          fileName: `environments/${relative(root, path).split(sep).join('/')}`,
          source: await readFile(path),
        });
    },
  };
}
