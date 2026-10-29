import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { stripTypeScriptTypes } from 'node:module';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { chromium } from 'playwright';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../../..'),
  moduleDirectory = process.env.FUJIMOD_DMP_WEBGPU_DIR,
  captureFile = process.env.FUJIMOD_DMP_CAPTURE_FILE;
if (!moduleDirectory || !captureFile)
  throw Error('Set FUJIMOD_DMP_WEBGPU_DIR and FUJIMOD_DMP_CAPTURE_FILE.');

const browserFiles = new Map([
  ['/tests/dmp-gpu-page.ts', resolve(repository, 'apps/web/tests/dmp-gpu-page.ts')],
  ['/tests/dmp-gpu-worker.ts', resolve(repository, 'apps/web/tests/dmp-gpu-worker.ts')],
]);
const server = createServer((request, response) => {
  void (async () => {
    response.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    response.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    const pathname = new URL(request.url ?? '/', 'http://127.0.0.1').pathname;
    if (pathname === '/') {
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      response.end(
        '<!doctype html><meta charset="utf-8"><title>DMP WebGPU proof</title><script type="module" src="/tests/dmp-gpu-page.ts"></script>',
      );
      return;
    }
    if (pathname.endsWith('.ts')) {
      const file = browserFiles.get(pathname);
      if (!file) {
        response.writeHead(404).end();
        return;
      }
      const source = await readFile(file, 'utf8');
      response.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8' });
      response.end(stripTypeScriptTypes(source));
      return;
    }
    const file =
      pathname === '/dmp_webgpu.js'
        ? resolve(moduleDirectory, 'dmp_webgpu.js')
        : pathname === '/dmp_webgpu_bg.wasm'
          ? resolve(moduleDirectory, 'dmp_webgpu_bg.wasm')
          : pathname === '/capture.dmpcap'
            ? captureFile
            : undefined;
    if (!file) {
      response.writeHead(404).end();
      return;
    }
    const contentType = pathname.endsWith('.wasm')
      ? 'application/wasm'
      : pathname.endsWith('.js')
        ? 'text/javascript; charset=utf-8'
        : 'application/octet-stream';
    response.writeHead(200, { 'content-type': contentType });
    response.end(await readFile(file));
  })().catch((error: unknown) => {
    response.writeHead(500).end(error instanceof Error ? error.message : String(error));
  });
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const address = server.address();
assert.ok(address && typeof address === 'object');

let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
try {
  browser = await chromium.launch({ headless: true, args: ['--enable-unsafe-webgpu'] });
  const page = await browser.newPage();
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${address.port}/`);
  await page.waitForFunction(() => document.body.dataset.proof, undefined, { timeout: 40000 });
  const text = await page.locator('pre').innerText();
  assert.equal(await page.locator('body').getAttribute('data-proof'), 'passed', text);
  const reply = JSON.parse(text) as {
    ok: boolean;
    error?: string;
    proof?: {
      adapter: string;
      matchesCpu: boolean;
      gpuDraws: number;
      gpuQueueSubmissions: number;
      cpuSnapshots: { address: string; length: number; sha256: string }[];
      gpuSnapshots: { address: string; length: number; sha256: string }[];
      cpuWrittenRanges: { address: string; length: number }[];
      gpuWrittenRanges: { address: string; length: number }[];
    };
  };
  assert.equal(reply.ok, true, JSON.stringify(reply));
  assert(reply.proof);
  assert(reply.proof.adapter.length > 0);
  assert.equal(reply.proof.gpuDraws, 1);
  assert.equal(reply.proof.gpuQueueSubmissions, 1);
  assert.equal(reply.proof.matchesCpu, true, JSON.stringify(reply.proof));
  assert.deepEqual(reply.proof.gpuSnapshots, reply.proof.cpuSnapshots);
  assert.deepEqual(reply.proof.gpuWrittenRanges, reply.proof.cpuWrittenRanges);
  assert.equal(reply.proof.cpuSnapshots.length, 1);
  assert.deepEqual(errors, []);
  console.log(
    `${reply.proof.adapter}: WebGPU matched the CPU output (${reply.proof.cpuSnapshots[0]!.length} bytes).`,
  );
} finally {
  await browser?.close();
  await new Promise<void>((resolveClose, reject) =>
    server.close((error) => (error ? reject(error) : resolveClose())),
  );
}
