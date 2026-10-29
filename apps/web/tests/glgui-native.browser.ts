import { chromium } from 'playwright';
import assert from 'node:assert/strict';

const base = process.env.FUJIMOD_WEB_URL;
const nativeWebSocket = process.env.FUJIMOD_NATIVE_WS_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL.');
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
  });
  const page = await context.newPage();
  page.setDefaultTimeout(20000);
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const url = new URL('/review/glgui-native/', base);
  if (nativeWebSocket) url.searchParams.set('ws', nativeWebSocket);
  await page.goto(url.href, { waitUntil: 'domcontentloaded' });
  try {
    await page.waitForFunction(
      () =>
        ['LIVE', 'ERROR'].includes(
          document.querySelector('#status')?.getAttribute('data-state') ?? '',
        ),
      undefined,
      { timeout: 44000 },
    );
  } catch (error) {
    const status = await page.locator('#status').textContent();
    throw Error(`Native preview did not reach LIVE (${status}): ${String(error)}`);
  }
  assert.equal(
    await page.locator('#status').getAttribute('data-state'),
    'LIVE',
    (await page.locator('#status').textContent()) ?? '',
  );
  await page.waitForFunction(() => window.nativeFrameCount >= 2, undefined, { timeout: 10000 });
  const canvas = page.locator('#lcd');
  assert.equal(await canvas.getAttribute('width'), '720');
  assert.equal(await canvas.getAttribute('height'), '480');
  const sequence = Number(await canvas.getAttribute('data-sequence'));
  await page.waitForFunction(
    (previous) => Number(document.querySelector('#lcd')?.getAttribute('data-sequence')) > previous,
    sequence,
    { timeout: 10000 },
  );
  await canvas.click({ position: { x: 360, y: 240 } });
  await page.waitForFunction(() => window.nativeTouchCount >= 2, undefined, { timeout: 10000 });
  assert.match((await canvas.getAttribute('data-last-touch')) ?? '', /"accepted":true/);
  await page.evaluate(async () => {
    const engine = (
      window as Window & {
        nativeEngine?: {
          press(id: string): Promise<void>;
          release(id: string): Promise<void>;
          dispose(): Promise<void>;
        };
      }
    ).nativeEngine;
    if (!engine) throw Error('Native engine was not installed on the review page.');
    await engine.press('down');
    await engine.release('down');
    await engine.dispose();
  });
  assert.deepEqual(errors, []);
  await context.close();
} finally {
  await browser.close();
}
console.log('Native GLGUI frame stream, LCD touch, and button input passed.');
