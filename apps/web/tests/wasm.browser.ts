import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the development server.');
const url = new URL('/__wasm-proof', base).href;
const output = new URL('../build/wasm-proof/', import.meta.url);
await mkdir(output, { recursive: true });
const types = [chromium, webkit].filter(
  (type) => !process.env.FUJIMOD_TEST_BROWSER || type.name() === process.env.FUJIMOD_TEST_BROWSER,
);
assert(types.length, 'Unknown FUJIMOD_TEST_BROWSER.');
for (const type of types) {
  const browser = await type.launch({ headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1100, height: 1000 } });
    const errors: string[] = [],
      requests: string[] = [],
      workers: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('request', (request) => requests.push(request.url()));
    page.on('worker', (worker) => workers.push(worker.url()));
    await page.goto(url);
    await page.waitForFunction(() => document.body.dataset.proof, undefined, { timeout: 60000 });
    const text = await page.locator('pre').innerText();
    assert.equal(await page.locator('body').getAttribute('data-proof'), 'passed', text);
    const result = JSON.parse(text) as {
      passed: boolean;
      frames: { sequence: number; pixels: number[] }[];
      metrics: { guestTicks: number; runtime: { instructions: string } };
      faults: { kind: string }[];
      card: { supported: boolean };
      session: { scenes: number[]; profile: { ticks: number }; refusals: Record<string, string> };
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
    assert.equal(result.card.supported, false);
    assert.deepEqual(result.session.scenes, [0, 1, 2]);
    assert.equal(result.session.profile.ticks, 20);
    assert.equal(Object.keys(result.session.refusals).length, 8);
    assert(workers.length >= 3 && workers.every((worker) => worker.includes('wasm-worker')));
    assert.deepEqual(
      requests.filter((request) =>
        /pyodide|numpy|pillow|sources\.zip|_native\.so/i.test(new URL(request).pathname),
      ),
      [],
    );
    assert.deepEqual(errors, []);
    await page.screenshot({ path: new URL(`${type.name()}.png`, output).pathname, fullPage: true });
    await writeFile(
      new URL(`${type.name()}.json`, output),
      JSON.stringify({ url, result, requests, workers }, null, 2) + '\n',
    );
    console.log(`${type.name()} Wasm execution proof passed: ${url}`);
  } finally {
    await browser.close();
  }
}
