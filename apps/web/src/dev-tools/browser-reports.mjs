// Real firmware crash/feedback reporting through the managed local development server.
import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { readFile, mkdir } from 'node:fs/promises';
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL');
const url = new URL(
  'dev-tools/?target=simulator&firmware=x70-110&connect=1&scenario=console-jpeg',
  base,
);
const engine = process.env.FUJIMOD_BROWSER === 'webkit' ? webkit : chromium;
const browser = await engine.launch();
function entries(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength),
    files = {};
  let offset = 0;
  while (view.getUint32(offset, true) === 0x04034b50) {
    const size = view.getUint32(offset + 18, true),
      length = view.getUint16(offset + 26, true),
      extra = view.getUint16(offset + 28, true);
    const name = new TextDecoder().decode(bytes.subarray(offset + 30, offset + 30 + length));
    const start = offset + 30 + length + extra;
    files[name] = bytes.subarray(start, start + size);
    offset = start + size;
  }
  return files;
}
try {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const saved = [],
    metadata = [];
  page.on('pageerror', (error) => console.error('PAGE ERROR', error.message));
  page.on('response', (response) => {
    const request = response.request();
    if (request.method() === 'POST' && response.url().endsWith('/__simulator-reports'))
      metadata.push(JSON.parse(request.postData()));
    if (request.method() === 'PUT' && response.url().includes('/__simulator-reports/')) {
      assert.equal(response.status(), 201);
      saved.push(response.url().split('/').at(-2));
    }
  });
  await page.goto(url.href);
  await page.waitForFunction(
    () => document.querySelector('.dt-status')?.textContent === 'Connection interrupted',
    null,
    { timeout: 240000 },
  );
  const deadline = Date.now() + 90000;
  while (!saved.length && Date.now() < deadline) await page.waitForTimeout(250);
  assert.ok(saved.length, await page.locator('.dt-workspace').innerText());
  const folder = new URL(`../../reports/${saved[0]}/`, import.meta.url);
  const manifest = JSON.parse(await readFile(new URL('metadata.json', folder), 'utf8'));
  assert.equal(manifest.source, 'dev-tools');
  assert.equal(manifest.event, 'crash');
  const files = entries(new Uint8Array(await readFile(new URL('report.zip', folder))));
  const report = JSON.parse(new TextDecoder().decode(files['report.json']));
  console.log('Snapshot:', JSON.stringify(report.snapshot));
  assert.ok(files['snapshot.zip'], JSON.stringify(report.snapshot));
  assert.equal(report.identity.firmwareId, 'x70-110');
  assert.equal(report.identity.nativeSha256.length, 64);
  assert.ok(report.logs.some((entry) => entry.message === 'dcfcc'));
  assert.ok(Object.keys(files).some((name) => name.startsWith('usb/')));
  await page.getByRole('button', { name: 'Feedback', exact: true }).click();
  await page.getByLabel('Feedback (optional)').fill('dcfcc stopped the simulator');
  await page.getByRole('button', { name: 'Save report', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'Saved locally' }).waitFor({ timeout: 90000 });
  assert.ok(saved.length >= 2);
  assert.equal(metadata.at(-1).description, 'dcfcc stopped the simulator');
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  await mkdir(new URL('../../build/dev-tools-reports/', import.meta.url), { recursive: true });
  await page.screenshot({
    path: new URL(`../../build/dev-tools-reports/${engine.name()}.png`, import.meta.url).pathname,
  });
  console.log(
    'PASS',
    engine.name(),
    'real firmware fault and manual feedback saved with snapshot, logs and identity:',
    saved.join(', '),
  );
} finally {
  await browser.close();
}
