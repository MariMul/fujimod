/** Qualify generated camera capabilities through the ordinary upload and panel UI. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import type { Catalog } from '../src/catalog';

declare global {
  interface Window {
    catalogState?: {
      ui?: { mode: number; cursor: number; menu_active: number };
      navigation?: {
        current_menu?: { pointer?: number; cursor?: number };
        physical_input_queue?: { pending: number; active: unknown; completed: number };
      };
      failure?: unknown;
      execution_error?: unknown;
      virtual_ticks?: number;
    };
    catalogError?: string;
  }
}
const url = process.env.FUJIMOD_WEB_URL;
const manifest = process.env.FUJIMOD_TEST_CATALOG;
const files: Record<string, string> = JSON.parse(process.env.FUJIMOD_TEST_FIRMWARE_FILES || '{}');
const output = process.env.FUJIMOD_TEST_OUTPUT;
const installed = process.env.FUJIMOD_TEST_INSTALLED_CATALOG === '1';
if (!url || !manifest || !output)
  throw Error('Set the web URL, generated catalog, DAT inputs and output directory.');
const catalog: Catalog = JSON.parse(await readFile(manifest, 'utf8'));
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
let failures = 0;
try {
  for (const firmware of catalog.firmwares.filter((entry) => entry.emulate)) {
    assert.ok(files[firmware.id], `Missing DAT input for ${firmware.id}`);
    const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
    await context.routeWebSocket('**/*', () => {});
    if (!installed)
      await context.route('**/runtime/catalog.json', (route) => route.fulfill({ json: catalog }));
    await context.route('**/local/firmware/**', (route) => route.fulfill({ status: 404 }));
    await context.addInitScript(() => {
      const OriginalWorker = window.Worker;
      window.Worker = class extends OriginalWorker {
        constructor(...args: ConstructorParameters<typeof Worker>) {
          super(...args);
          this.addEventListener(
            'message',
            ({
              data,
            }: MessageEvent<{
              state?: Window['catalogState'];
              type?: string;
              error?: string;
              message?: string;
            }>) => {
              if (data.state?.ui) window.catalogState = data.state;
              if (data.type === 'error') window.catalogError = data.error;
              if (data.type === 'progress') console.log(`catalog progress: ${data.message}`);
            },
          );
        }
      };
    });
    const page = await context.newPage();
    if (installed) {
      const response: import('playwright').APIResponse = await page.request.get(
        new URL('runtime/catalog.json', url).href,
      );
      assert.equal(response.status(), 200);
      const served: Catalog = await response.json();
      assert.deepEqual(
        served.firmwares.find((entry) => entry.id === firmware.id),
        firmware,
      );
    }
    page.setDefaultTimeout(180000);
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') console.error(message.text());
      if (message.text().startsWith('catalog progress:')) console.log(message.text());
    });
    try {
      console.log(`${firmware.id}: opening website`);
      await page.goto(url);
      console.log(`${firmware.id}: selecting firmware`);
      const chooser = page.waitForEvent('filechooser');
      await page
        .getByRole('button', { name: `Try ${firmware.model} emulator`, exact: true })
        .click();
      console.log(`${firmware.id}: choosing DAT`);
      await (await chooser).setFiles(files[firmware.id]);
      console.log(`${firmware.id}: waiting for cold boot`);
      await page.waitForFunction(
        () =>
          window.catalogError ||
          document.querySelector<HTMLElement>('#fault-heading')?.checkVisibility() ||
          document.querySelector('#controls button[data-id="menu_ok"]'),
      );
      assert.equal(await page.evaluate(() => window.catalogError), undefined);
      assert.equal(
        await page.locator('#fault-heading').isVisible(),
        false,
        await page.locator('#status').innerText(),
      );
      async function settled() {
        await page.waitForFunction(() => {
          const queue = window.catalogState?.navigation?.physical_input_queue;
          return window.catalogError || (queue && !queue.pending && !queue.active);
        });
        assert.equal(await page.evaluate(() => window.catalogError), undefined);
      }
      async function waitTicks(ticks: number) {
        const start = await page.evaluate(() => window.catalogState?.virtual_ticks ?? 0);
        await page.waitForFunction(
          (target) => window.catalogError || (window.catalogState?.virtual_ticks ?? 0) >= target,
          start + ticks,
        );
        assert.equal(await page.evaluate(() => window.catalogError), undefined);
      }
      async function press(id: string) {
        const before = await page.evaluate(
          () => window.catalogState?.navigation?.physical_input_queue?.completed ?? 0,
        );
        await page.locator(`#controls button[data-id="${id}"]`).click();
        await page.waitForFunction(
          (count) =>
            window.catalogError ||
            (window.catalogState?.navigation?.physical_input_queue?.completed ?? 0) >= count + 2,
          before,
        );
        await settled();
        // Readiness is observed from the running guest, with no UI fixture.
        await waitTicks(6_000_000);
        console.log(
          `${firmware.id}: ${id} ${JSON.stringify(await page.evaluate(() => window.catalogState?.ui))}`,
        );
      }
      await settled();
      await waitTicks(6_000_000);
      for (let pageNumber = 0; pageNumber < 16; pageNumber++) {
        if (await page.evaluate(() => window.catalogState?.ui?.mode === 3)) break;
        await press('menu_ok');
      }
      await page.waitForFunction(() => window.catalogState?.ui?.mode === 3);
      await waitTicks(6_000_000);
      await press('menu_ok');
      await page.waitForFunction(
        () =>
          window.catalogState?.ui?.menu_active === 1 ||
          !!window.catalogState?.navigation?.current_menu?.pointer,
      );
      const before = await page.evaluate(
        () =>
          window.catalogState?.ui?.cursor ?? window.catalogState?.navigation?.current_menu?.cursor,
      );
      await page.screenshot({ path: `${output}/${firmware.id}-menu.png`, fullPage: true });
      await press('down');
      await page.waitForFunction(
        (cursor) =>
          (window.catalogState?.ui?.cursor ??
            window.catalogState?.navigation?.current_menu?.cursor) !== cursor,
        before,
      );
      const after = await page.evaluate(
        () =>
          window.catalogState?.ui?.cursor ?? window.catalogState?.navigation?.current_menu?.cursor,
      );
      assert.notEqual(before, after);
      await press('disp');
      assert.deepEqual(errors, []);
      const state = await page.evaluate(() => window.catalogState);
      assert.equal(state?.failure, null);
      assert.equal(state?.execution_error, null);
      await writeFile(
        `${output}/${firmware.id}-browser.json`,
        JSON.stringify({
          firmware: firmware.id,
          sha256: firmware.sha256,
          cursor: [before, after],
          passed: true,
        }),
      );
      console.log(`${firmware.id}: browser boot, first-start, MENU, cursor and BACK passed`);
    } catch (error) {
      await page.screenshot({ path: `${output}/${firmware.id}-failure.png`, fullPage: true });
      console.error(await page.locator('body').innerText());
      await writeFile(
        `${output}/${firmware.id}-browser.json`,
        JSON.stringify({ firmware: firmware.id, passed: false, error: String(error) }),
      );
      failures++;
    }
    await context.close();
  }
  assert.equal(failures, 0, 'Every advertised camera must pass its website workflow');
} finally {
  await browser.close();
}
