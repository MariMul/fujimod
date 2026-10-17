// Full firmware integration; requires a configured local runtime and DAT.
import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the configured development server.');
const url = new URL('dev-tools/', base);
url.searchParams.set('target', 'simulator');
url.searchParams.set('firmware', process.env.FUJIMOD_FIRMWARE ?? 'x70-110');
url.searchParams.set('connect', '1');
const engine = process.env.FUJIMOD_BROWSER === 'webkit' ? webkit : chromium;
const browser = await engine.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = [];
  page.on('pageerror', (error) => {
    errors.push(error.message);
    console.error(error.message);
  });
  await page.addInitScript(() => {
    if ('usb' in navigator)
      Object.defineProperty(navigator, 'usb', {
        configurable: true,
        value: Object.assign(new EventTarget(), {
          getDevices() {
            throw Error('Physical USB forbidden in firmware test');
          },
          requestDevice() {
            throw Error('Physical USB forbidden in firmware test');
          },
        }),
      });
  });
  await page.goto(url.href);
  await page.waitForFunction(
    () =>
      document.querySelector('.dt-status')?.textContent === 'Connected' ||
      Boolean(document.querySelector('.level-error')),
    null,
    { timeout: 240000 },
  );
  assert.equal(
    await page.locator('.dt-status').textContent(),
    'Connected',
    await page.locator('.dt-workspace').innerText(),
  );
  await page.getByRole('button', { name: 'USB', exact: true }).click();
  await page.getByText('Control Open completed', { exact: false }).first().waitFor();
  await page.getByLabel('Console command', { exact: true }).fill('hello');
  await page.getByRole('button', { name: 'Send ↵', exact: true }).click();
  await page.getByText('Hello, world!', { exact: false }).first().waitFor({ timeout: 30000 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  assert.deepEqual(errors, []);
  await mkdir(new URL('../../build/dev-tools-emulator/', import.meta.url), { recursive: true });
  await page.screenshot({
    path: new URL(`../../build/dev-tools-emulator/${engine.name()}.png`, import.meta.url).pathname,
    fullPage: true,
  });
  await page.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelector('.dt-status')?.textContent === 'Disconnected',
  );
  console.log(
    `PASS ${engine.name()} actual firmware: cold boot → WebUSB descriptors → nusb → FF80 → stock hello console response`,
  );
} catch (error) {
  for (const page of browser.contexts().flatMap((c) => c.pages()))
    console.error(
      await page
        .locator('.dt-workspace')
        .innerText()
        .catch(() => ''),
    );
  throw error;
} finally {
  await browser.close();
}
