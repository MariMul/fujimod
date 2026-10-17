import { defineConfig, loadEnv, type Plugin, type Connect } from 'vite';
import { execFileSync } from 'node:child_process';
import { createReadStream, existsSync } from 'node:fs';
import { createGzip } from 'node:zlib';
import { stat } from 'node:fs/promises';
import { resolve, sep } from 'node:path';

// Opt-in local artifact mounts are served by the local server, never copied into dist.
function localArtifacts(env: Record<string, string>): Plugin {
  const configure = (middlewares: Connect.Server) => {
    const firmwares = Object.entries(JSON.parse(env.FUJIMOD_LOCAL_FIRMWARES || '{}'));
    if (firmwares.some(([id, path]) => !/^[a-z0-9-]+$/.test(id) || typeof path !== 'string'))
      throw Error('FUJIMOD_LOCAL_FIRMWARES must map firmware IDs to local DAT paths');
    const mounts = [
      ['/runtime/', env.FUJIMOD_RUNTIME],
      ['/artifacts/', env.FUJIMOD_PATCHES],
      ['/local/firmware.dat', env.FUJIMOD_DAT],
      ...firmwares.map(([id, path]) => [`/local/firmware/${id}.dat`, path as string]),
    ].filter((pair): pair is [string, string] => Boolean(pair[1]));
    middlewares.use(async (req, res, next) => {
      const url = new URL(req.url || '/', 'http://localhost').pathname;
      const mount = mounts.find(([prefix]) => url.startsWith(prefix));
      if (!mount) return next();
      const [prefix, root] = mount;
      let relative: string;
      try { relative = decodeURIComponent(url.slice(prefix.length)); }
      catch { res.statusCode = 400; res.end(); return; }
      if (prefix === '/runtime/' && !/^(catalog\.json|[a-zA-Z0-9_-]+\/(pyodide\/[a-zA-Z0-9_.-]+|sources\.zip|_native\.so|inputs\.json|firmware-inputs\.zip))$/.test(relative)) {
        res.statusCode = 404; res.end(); return;
      }
      const path = relative ? resolve(root, relative) : resolve(root);
      if (relative && !path.startsWith(resolve(root) + sep)) { res.statusCode = 403; res.end(); return; }
      try {
        const info = await stat(path);
        if (!info.isFile()) throw Error('not a file');
        const gzip = /\bgzip\b/.test(` ${req.headers['accept-encoding'] || ''}`) && info.size > 65536 && /\.(dat|so|wasm|mjs|js)$/i.test(path);
        const etag = `"${info.size.toString(16)}-${info.mtimeMs.toString(16)}${gzip ? '-gzip' : ''}"`;
        res.setHeader('Vary', 'Accept-Encoding');
        res.setHeader('Cache-Control', 'no-cache');
        res.setHeader('ETag', etag);
        if (req.headers['if-none-match'] === etag) { res.statusCode = 304; res.end(); return; }
        if (gzip) res.setHeader('Content-Encoding', 'gzip');
        else res.setHeader('Content-Length', info.size);
        res.setHeader('Content-Type', path.endsWith('.mjs') || path.endsWith('.js') ? 'text/javascript' : path.endsWith('.json') ? 'application/json' : path.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream');
        if (req.method === 'HEAD') { res.end(); return; }
        const stream = createReadStream(path);
        (gzip ? stream.pipe(createGzip({level:1})) : stream).pipe(res);
      } catch { res.statusCode = 404; res.end(); }
    });
  };
  return { name: 'local-artifacts', apply: 'serve', configureServer: server => configure(server.middlewares), configurePreviewServer: server => configure(server.middlewares) };
}
export default defineConfig(({ mode, command }) => ({
  base: './', define: { __FUJIMOD_SIMULATOR_BUILD__: JSON.stringify(`${execFileSync('git', ['rev-parse', '--short=12', 'HEAD'], {encoding:'utf8'}).trim()} / ${new Date().toISOString()}`), __FUJIMOD_LOCAL_DAT__: (command === 'serve' || mode === 'local-preview') && Boolean(loadEnv(mode, process.cwd(), 'FUJIMOD_').FUJIMOD_DAT), __FUJIMOD_CATALOG__: JSON.stringify(existsSync('public/runtime/catalog.json') ? 'runtime/catalog.json' : 'catalog.json') }, plugins: [localArtifacts(loadEnv(mode, process.cwd(), 'FUJIMOD_'))],
  server: { host: '127.0.0.1', fs: { strict: true } },
  preview: { host: '127.0.0.1', allowedHosts: ['cozycat.tail2b5ca.ts.net'] },
  build: { target: 'es2022', outDir: mode === 'local-preview' ? 'dist-local-preview' : 'dist' }, worker: { format: 'es' },
}));
