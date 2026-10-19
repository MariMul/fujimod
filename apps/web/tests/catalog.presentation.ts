/** Check the served catalog's badges and launch actions in the camera table. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import type { Catalog } from '../src/catalog';

const url = process.env.FUJIMOD_WEB_URL;
const output = process.env.FUJIMOD_TEST_OUTPUT;
if (!url || !output) throw Error('Set the web URL and screenshot output directory.');
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1500, height: 1100 } });
  await page.context().routeWebSocket('**/*', () => {});
  const response = await page.request.get(new URL('runtime/catalog.json', url).href);
  assert.equal(response.status(), 200);
  const catalog: Catalog = await response.json();
  await page.goto(new URL('#camera-table', url).href);
  await page.waitForFunction(() => performance.getEntriesByName('camera-catalog-ready').length);
  const expected = catalog.firmwares.filter((firmware) => firmware.emulate);
  for (const firmware of expected) {
    const row = page.locator('.cam').filter({
      has: page.getByRole('button', { name: `Try ${firmware.model} emulator`, exact: true }),
    });
    assert.equal(await row.count(), 1, firmware.id);
    assert.equal(await row.locator('.chip.emu').count(), 1, firmware.id);
    assert.equal(
      await row.locator('.chip.emu.early').count(),
      Number(firmware.emulate?.maturity === 'early'),
      firmware.id,
    );
    assert.equal(await row.getByRole('button').isEnabled(), true, firmware.id);
  }
  assert.equal(await page.locator('.chip.emu').count(), expected.length);
  await page.locator('#camera-table').screenshot({ path: `${output}/catalog.png` });
  console.log(`${expected.length} catalog cameras: badges and launch actions verified at ${url}`);
} finally {
  await browser.close();
}
