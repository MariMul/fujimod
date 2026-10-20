import type { Plugin } from 'vite';
/** Development-only executable proof, never a production route or fixture screen. */
export function wasmProofRoute(): Plugin {
  return {
    name: 'wasm-proof-route',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        if (new URL(request.url || '/', 'http://localhost').pathname !== '/__wasm-proof')
          return next();
        response.setHeader('Content-Type', 'text/html');
        response.end(
          '<!doctype html><html lang="en"><meta charset="utf-8"><title>Wasm execution proof</title><body><script type="module" src="/tests/wasm-proof.ts"></script></body></html>',
        );
      });
    },
  };
}
