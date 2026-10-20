import type { Plugin } from 'vite';
import { createReadStream, statSync } from 'node:fs';
import { createGzip } from 'node:zlib';
/** Development-only executable proof, never a production route or fixture screen. */
export function wasmProofRoute(env: Record<string, string> = {}): Plugin {
  return {
    name: 'wasm-proof-route',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const path = new URL(request.url || '/', 'http://localhost').pathname;
        const artifact =
          path === '/__wasm-camera/wasm.json'
            ? env.FUJIMOD_WASM_CAMERA
            : path === '/__wasm-camera/firmware.dat'
              ? env.FUJIMOD_WASM_DAT
              : undefined;
        if (artifact) {
          if (!['GET', 'HEAD'].includes(request.method || 'GET')) {
            response.statusCode = 405;
            response.end();
            return;
          }
          try {
            const size = statSync(artifact).size;
            const gzip = /\bgzip\b/.test(request.headers['accept-encoding'] || '');
            response.setHeader('Vary', 'Accept-Encoding');
            if (gzip) response.setHeader('Content-Encoding', 'gzip');
            else response.setHeader('Content-Length', size);
            response.setHeader(
              'Content-Type',
              path.endsWith('.json') ? 'application/json' : 'application/octet-stream',
            );
            response.setHeader('Cache-Control', 'no-store');
            if (request.method === 'HEAD') response.end();
            else {
              const stream = createReadStream(artifact).on('error', () => response.destroy());
              (gzip
                ? stream.pipe(createGzip({ level: 1 })).on('error', () => response.destroy())
                : stream
              ).pipe(response);
            }
          } catch {
            response.statusCode = 404;
            response.end('Artifact unavailable.');
          }
          return;
        }
        if (path === '/__wasm-camera/wasm.json' || path === '/__wasm-camera/firmware.dat') {
          response.statusCode = 404;
          response.end('Artifact unavailable.');
          return;
        }
        if (
          path !== '/__wasm-proof' &&
          path !== '/__wasm-camera' &&
          path !== '/__wasm-lifecycle' &&
          path !== '/__wasm-phases'
        )
          return next();
        const script =
          path === '/__wasm-phases'
            ? 'wasm-phases-proof'
            : path === '/__wasm-lifecycle'
              ? 'wasm-lifecycle-proof'
              : path === '/__wasm-camera'
                ? 'wasm-camera-proof'
                : 'wasm-proof';
        response.setHeader('Content-Type', 'text/html');
        response.end(
          `<!doctype html><html lang="en"><meta charset="utf-8"><title>Wasm execution proof</title><body><script type="module" src="/tests/${script}.ts"></script></body></html>`,
        );
      });
    },
  };
}
