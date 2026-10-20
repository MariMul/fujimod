import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { availableParallelism, loadavg } from 'node:os';
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the prepared camera server.');
const url = new URL('/__wasm-phases', base).href;
const output = new URL('../build/wasm-phases/', import.meta.url);
await mkdir(output, { recursive: true });
const failures: string[] = [];
for (const type of [chromium, webkit].filter(
  (type) => !process.env.FUJIMOD_TEST_BROWSER || type.name() === process.env.FUJIMOD_TEST_BROWSER,
)) {
  const browser = await type.launch({ headless: true });
  const host = {
    startedAt: new Date().toISOString(),
    cpus: availableParallelism(),
    loadBefore: loadavg(),
    loadAfter: [] as number[],
  };
  const phaseLoads: { phase: string; stage: string; load: number[] }[] = [];
  try {
    const context = await browser.newContext({ viewport: { width: 1200, height: 1000 } });
    await context.routeWebSocket('**/*', () => {});
    const page = await context.newPage(),
      errors: string[] = [],
      requests: string[] = [];
    page.on('console', (message) => {
      const text = message.text();
      if (text.startsWith('{"phase":')) {
        const sample = JSON.parse(text) as { phase: string; stage: string };
        phaseLoads.push({ phase: sample.phase, stage: sample.stage, load: loadavg() });
      }
      console.log(type.name() + ': ' + text);
    });
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('request', (request) => requests.push(request.url()));
    await page.goto(url);
    await page.waitForFunction(() => document.body.dataset.proof, undefined, { timeout: 1200000 });
    const result = JSON.parse(await page.locator('pre').innerText());
    host.loadAfter = loadavg();
    await page.screenshot({ path: new URL(`${type.name()}.png`, output).pathname, fullPage: true });
    await writeFile(
      new URL(`${type.name()}.json`, output),
      JSON.stringify({ url, host, phaseLoads, result, requests, errors }, null, 2) + '\n',
    );
    assert.equal(
      await page.locator('body').getAttribute('data-proof'),
      'passed',
      JSON.stringify({
        error: result.error,
        phases: result.phases?.map(
          ({
            name,
            speed,
            completed,
            error,
          }: {
            name: string;
            speed: number;
            completed: boolean;
            error?: string;
          }) => ({ name, speed, completed, error }),
        ),
      }),
    );
    assert.deepEqual(errors, []);
    assert.deepEqual(
      requests.filter((request) =>
        /pyodide|sources\.zip|_native\.so/i.test(new URL(request).pathname),
      ),
      [],
    );
    console.log(`${type.name()}: every warm operating phase reached at least 1.0x.`);
  } catch (error) {
    host.loadAfter = loadavg();
    await writeFile(
      new URL(`${type.name()}-failure.json`, output),
      JSON.stringify({ url, host, phaseLoads, error: String(error) }, null, 2) + '\n',
    );
    failures.push(`${type.name()}: ${String(error)}`);
  } finally {
    await browser.close();
  }
}

assert.deepEqual(failures, []);
