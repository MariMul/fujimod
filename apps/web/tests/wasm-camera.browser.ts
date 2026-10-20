import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
const base = process.env.FUJIMOD_WEB_URL,
  expected = process.env.FUJIMOD_WASM_EXPECT_REFUSAL;
if (!base || !expected)
  throw Error(
    'Set FUJIMOD_WEB_URL and FUJIMOD_WASM_EXPECT_REFUSAL for the current camera boundary.',
  );
const url = new URL('/__wasm-camera', base).href;
const output = new URL('../build/wasm-camera/', import.meta.url);
await mkdir(output, { recursive: true });
for (const type of [chromium, webkit]) {
  const browser = await type.launch({ headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1200, height: 1000 } });
    const errors: string[] = [],
      requests: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('request', (request) => requests.push(request.url()));
    await page.goto(url);
    await page.waitForFunction(() => document.body.dataset.proof, undefined, { timeout: 120000 });
    const text = await page.locator('pre').innerText();
    assert.equal(await page.locator('body').getAttribute('data-proof'), 'refused', text);
    const result = JSON.parse(text) as {
      firmwareId: string;
      datBytes: number;
      datSha256: string;
      created: boolean;
      status: unknown;
      frame: unknown;
      frames: number;
      metrics: number;
      faults: { kind: string; details: { error: string } }[];
    };
    assert.equal(result.firmwareId, process.env.FUJIMOD_WASM_FIRMWARE);
    assert.equal(result.datSha256, process.env.FUJIMOD_WASM_DAT_SHA256);
    assert(result.datBytes > 0);
    assert.equal(result.created, false);
    assert.equal(result.status, null);
    assert.equal(result.frame, null);
    assert.equal(result.frames, 0);
    assert.equal(result.metrics, 0);
    assert(
      result.faults.some(
        (fault) => fault.kind === 'unsupported-device' && fault.details.error === expected,
      ),
      text,
    );
    assert.deepEqual(errors, []);
    assert.deepEqual(
      requests.filter((request) =>
        /pyodide|numpy|pillow|sources\.zip|_native\.so/i.test(new URL(request).pathname),
      ),
      [],
    );
    await page.screenshot({ path: new URL(`${type.name()}.png`, output).pathname, fullPage: true });
    await writeFile(
      new URL(`${type.name()}.json`, output),
      JSON.stringify({ url, result, requests }, null, 2) + '\n',
    );
    console.log(
      `${type.name()}: ${result.firmwareId} reaches its expected construction refusal; no guest board exists.`,
    );
  } finally {
    await browser.close();
  }
}
