import { contentSecurityPolicy } from './src/security';
import { defineConfig } from 'vite-plus';
import { loadEnv, type Plugin, type Connect } from 'vite';
import solid from '@solidjs/vite-plugin';
import { wasmProofRoute } from './scripts/wasm-proof-route';
import { simulatorReports } from './scripts/local-reports.ts';
import { bugReportRoute } from './src/bug/route.ts';
import { browserDiagnostics } from './scripts/browser-diagnostics.ts';
import { execFileSync } from 'node:child_process';
import { createReadStream, existsSync, readFileSync } from 'node:fs';
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
      ['/artifacts/docs/', env.FUJIMOD_MEDIA],
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
      try {
        relative = decodeURIComponent(url.slice(prefix.length));
      } catch {
        res.statusCode = 400;
        res.end();
        return;
      }
      if (
        prefix === '/runtime/' &&
        !/^(catalog\.json|[a-zA-Z0-9_-]+\/(wasm\.json|camera\.mjs|camera\.wasm|runtime-NOTICES\.txt))$/.test(
          relative,
        )
      ) {
        res.statusCode = 404;
        res.end();
        return;
      }
      const path = relative ? resolve(root, relative) : resolve(root);
      if (relative && !path.startsWith(resolve(root) + sep)) {
        res.statusCode = 403;
        res.end();
        return;
      }
      try {
        const info = await stat(path);
        if (!info.isFile()) throw Error('not a file');
        const gzip =
          /\bgzip\b/.test(` ${req.headers['accept-encoding'] || ''}`) &&
          info.size > 65536 &&
          /\.(dat|so|wasm|mjs|js)$/i.test(path);
        const etag = `"${info.size.toString(16)}-${info.mtimeMs.toString(16)}${gzip ? '-gzip' : ''}"`;
        res.setHeader('Vary', 'Accept-Encoding');
        res.setHeader('Cache-Control', 'no-cache');
        res.setHeader('ETag', etag);
        if (req.headers['if-none-match'] === etag) {
          res.statusCode = 304;
          res.end();
          return;
        }
        if (gzip) res.setHeader('Content-Encoding', 'gzip');
        // Mobile Safari fetches video in byte ranges and stalls without them.
        const range = gzip ? null : /^bytes=(\d*)-(\d*)$/.exec(req.headers.range || '');
        const first = range ? (range[1] ? Number(range[1]) : info.size - Number(range[2])) : 0;
        const last =
          range && range[1] && range[2] ? Math.min(Number(range[2]), info.size - 1) : info.size - 1;
        if (range && (first > last || first < 0)) {
          res.statusCode = 416;
          res.setHeader('Content-Range', `bytes */${info.size}`);
          res.end();
          return;
        }
        if (!gzip) {
          res.setHeader('Content-Length', last - first + 1);
          res.setHeader('Accept-Ranges', 'bytes');
        }
        if (range) {
          res.statusCode = 206;
          res.setHeader('Content-Range', `bytes ${first}-${last}/${info.size}`);
        }
        res.setHeader(
          'Content-Type',
          path.endsWith('.mjs') || path.endsWith('.js')
            ? 'text/javascript'
            : path.endsWith('.json')
              ? 'application/json'
              : path.endsWith('.wasm')
                ? 'application/wasm'
                : path.endsWith('.mp4')
                  ? 'video/mp4'
                  : path.endsWith('.webp')
                    ? 'image/webp'
                    : 'application/octet-stream',
        );
        if (req.method === 'HEAD') {
          res.end();
          return;
        }
        const stream = createReadStream(path, { start: first, end: last });
        (gzip ? stream.pipe(createGzip({ level: 1 })) : stream).pipe(res);
      } catch {
        res.statusCode = 404;
        res.end();
      }
    });
  };
  return {
    name: 'local-artifacts',
    apply: 'serve',
    configureServer: (server) => configure(server.middlewares),
    configurePreviewServer: (server) => configure(server.middlewares),
  };
}
// Feature media rendered for the catalog: an opt-in local directory, else the staged site assets.
function featureMedia(env: Record<string, string>) {
  const path = env.FUJIMOD_MEDIA
    ? resolve(env.FUJIMOD_MEDIA, 'media.json')
    : 'public/artifacts/docs/media.json';
  return existsSync(path) ? readFileSync(path, 'utf8') : 'null';
}

export default defineConfig(({ mode, command }) => ({
  base: '/',
  // Solid's server build consumes the manifest produced by its client build.
  builder: { sharedPlugins: true },
  define: {
    __FUJIMOD_BUILD_DAY__: JSON.stringify(new Date().toISOString().slice(0, 10)),
    __FUJIMOD_INITIAL_CATALOG__: readFileSync(
      existsSync('public/runtime/catalog.json')
        ? 'public/runtime/catalog.json'
        : 'public/catalog.json',
      'utf8',
    ),
    __FUJIMOD_INITIAL_RELEASE__: existsSync('public/artifacts/patches.json')
      ? readFileSync('public/artifacts/patches.json', 'utf8')
      : 'null',
    __FUJIMOD_SIMULATOR_BUILD__: JSON.stringify(
      `${execFileSync('git', ['rev-parse', '--short=12', 'HEAD'], { encoding: 'utf8' }).trim()} / ${new Date().toISOString()}`,
    ),
    __FUJIMOD_SIGNAL_URL__: JSON.stringify(
      loadEnv(mode, process.cwd(), 'FUJIMOD_').FUJIMOD_SIGNAL_URL || '',
    ),
    __FUJIMOD_FEATURE_MEDIA__: featureMedia(loadEnv(mode, process.cwd(), 'FUJIMOD_')),
    __FUJIMOD_LOCAL_DAT__:
      (command === 'serve' || mode === 'local-preview') &&
      Boolean(loadEnv(mode, process.cwd(), 'FUJIMOD_').FUJIMOD_DAT),
    __FUJIMOD_CATALOG__: JSON.stringify(
      existsSync('public/runtime/catalog.json') ? 'runtime/catalog.json' : 'catalog.json',
    ),
  },
  plugins: [
    solid({ start: { middleware: './src/middleware.ts' }, ssr: true }),
    bugReportRoute(),
    wasmProofRoute(loadEnv(mode, process.cwd(), 'FUJIMOD_')),
    simulatorReports(),
    browserDiagnostics(),
    localArtifacts(loadEnv(mode, process.cwd(), 'FUJIMOD_')),
    {
      name: 'web-output-directories',
      enforce: 'post',
      config() {
        const output = mode === 'local-preview' ? 'dist-local-preview' : 'dist';
        return {
          build: { outDir: `${output}/client` },
          environments: {
            client: { build: { outDir: `${output}/client` } },
            ssr: { build: { outDir: `${output}/server` } },
          },
        };
      },
    },
  ],
  lint: {
    jsPlugins: ['eslint-plugin-solid'],
    settings: { solid: { version: 2 } },
    overrides: [
      {
        files: ['**/*.{jsx,tsx}'],
        rules: {
          'solid/reactivity': 'error',
          'solid/no-destructure': 'error',
          'solid/components-return-once': 'error',
          'solid/no-react-deps': 'error',
          'solid/removed-api': 'error',
          'solid/no-single-arg-create-effect': 'error',
          'solid/no-accessor-as-prop': 'error',
          'solid/no-store-mutation-outside-setter': 'error',
          'solid/no-write-in-pure-computation': 'error',
        },
      },
    ],
    options: { typeAware: true, typeCheck: true },
    ignorePatterns: [
      'build/**',
      'dist/**',
      'dist-local-preview/**',
      'public/runtime/**',
      'public/artifacts/**',
    ],
  },
  fmt: {
    singleQuote: true,
    ignorePatterns: [
      'build/**',
      'dist/**',
      'dist-local-preview/**',
      'public/runtime/**',
      'public/artifacts/**',
    ],
  },
  test: {
    pool: 'forks',
    projects: [
      {
        extends: true,
        test: {
          name: 'unit',
          environment: 'node',
          include: ['src/**/*.test.ts', 'tests/**/*.test.ts'],
        },
      },
      {
        extends: true,
        test: {
          name: 'components',
          environment: 'jsdom',
          include: ['src/**/*.test.tsx'],
        },
      },
    ],
  },
  server: {
    headers: {
      'Content-Security-Policy': contentSecurityPolicy,
    },
    allowedHosts: ['.ts.net'],
    watch: {
      ignored: [
        '**/build/**',
        '**/dist/**',
        '**/dist-local-preview/**',
        '**/browser-bug-reports/**',
      ],
    },
    proxy: { '/usb-proxy': { target: 'http://127.0.0.1:5202', ws: true } },
    host: '127.0.0.1',
    fs: { strict: true, allow: [resolve('../..')] },
  },
  preview: {
    headers: {
      'Content-Security-Policy': contentSecurityPolicy,
    },
    proxy: { '/usb-proxy': { target: 'http://127.0.0.1:5202', ws: true } },
    host: '127.0.0.1',
    allowedHosts: ['.ts.net'],
  },
  build: { target: 'es2022' },
  worker: { format: 'es' },
}));
