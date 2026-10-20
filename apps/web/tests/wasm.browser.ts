import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the development server.');
const url = new URL('/__wasm-proof', base).href;
const output = new URL('../build/wasm-proof/', import.meta.url);
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 1000 } });
  const errors: string[] = [],
    requests: string[] = [],
    workers: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('request', (request) => requests.push(request.url()));
  page.on('worker', (worker) => workers.push(worker.url()));
  await page.goto(url);
  await page.waitForFunction(() => document.body.dataset.proof, { timeout: 60000 });
  const text = await page.locator('pre').innerText();
  assert.equal(await page.locator('body').getAttribute('data-proof'), 'passed', text);
  const result = JSON.parse(text) as {
    passed: boolean;
    frames: { sequence: number; pixels: number[] }[];
    metrics: { guestTicks: number; runtime: { instructions: string } };
    faults: { kind: string }[];
    missing: string;
  };
  assert(result.passed);
  assert.deepEqual(
    result.frames.map((frame) => frame.sequence),
    [1, 2],
  );
  assert.deepEqual(result.frames[0]!.pixels, Array(4).fill([0, 1, 35, 255]).flat());
  assert.deepEqual(result.frames[1]!.pixels, Array(4).fill([0, 4, 86, 255]).flat());
  assert.equal(result.metrics.guestTicks, 7500000);
  assert.equal(result.metrics.runtime.instructions, '7500000');
  assert.equal(result.faults[0]!.kind, 'guest-fault');
  assert.match(result.missing, /camera_command/);
  assert(workers.length >= 3 && workers.every((worker) => worker.includes('wasm-worker')));
  assert.deepEqual(
    requests.filter((request) =>
      /pyodide|numpy|pillow|sources\.zip|_native\.so/i.test(new URL(request).pathname),
    ),
    [],
  );
  assert.deepEqual(errors, []);
  await page.screenshot({ path: new URL('chromium.png', output).pathname, fullPage: true });
  await writeFile(
    new URL('chromium.json', output),
    JSON.stringify({ url, result, requests, workers }, null, 2) + '\n',
  );
  console.log(`Chromium Wasm execution proof passed: ${url}`);
} finally {
  await browser.close();
}
