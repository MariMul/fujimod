/** Review every catalog SD sequence at phone width through the managed development server. */
import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the managed web URL.');
const url = new URL('/review/power-combos/', base).href;
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 375, height: 900 } });
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });
  const response = await page.goto(url);
  assert.equal(response?.status(), 200);
  await page.locator('[data-body="x70"] .power-combo-keys').waitFor();
  assert.equal(await page.locator('[data-body]').count(), 49);
  assert.equal(await page.locator('[data-body="xt5"] h2').innerText(), 'X-T5');
  assert.equal(await page.locator('[data-body="xt30ii"] h2').innerText(), 'X-T30 II');
  assert.equal(await page.locator('[data-body="xq"] h2').innerText(), 'XQ1 / XQ2');
  const x70 = page.locator('[data-body="x70"]');
  assert.equal(
    await x70.locator('.rocker-drawing[data-held="press"] .drawing-arrow.push').count(),
    1,
  );
  assert.equal(await x70.locator('.dial-drawing').count(), 0);
  assert.equal(await x70.locator('.key.rec').count(), 1);
  assert.equal(await x70.locator('[data-part="positions"]').count(), 0);
  const xt5 = page.locator('[data-body="xt5"]');
  assert.equal(await xt5.locator('.dial-drawing[data-held="press"]').count(), 2);
  const overflow = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('.phone, .power-combo-keys')]
      .filter((element) => element.scrollWidth > element.clientWidth + 1)
      .map((element) => element.closest<HTMLElement>('[data-body]')?.dataset.body),
  );
  assert.deepEqual(overflow, []);
  assert.deepEqual(errors, []);
  console.log(`49 bodies reviewed without overflow or browser errors: ${url}`);
} finally {
  await browser.close();
}
