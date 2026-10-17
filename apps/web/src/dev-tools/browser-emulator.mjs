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
  page.on('framenavigated', (frame) => {
    if (frame === page.mainFrame()) console.log('Navigation:', frame.url());
  });
  page.on('crash', () => console.error('Browser renderer crashed'));
  page.on('websocket', (socket) =>
    socket.on('framereceived', ({ payload }) => {
      if (String(payload).includes('full-reload')) console.log('Reload:', payload);
    }),
  );
  const usbWasmRequests = [];
  page.on('request', (request) => {
    if (request.url().endsWith('/fujimod_bg.wasm')) usbWasmRequests.push(request.url());
  });
  const memoryCheck = process.env.FUJIMOD_MEMORY_CHECK === '1';
  if (memoryCheck) {
    await page.route('**/emulator-worker.ts*', async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        body:
          (await response.text()) +
          `
self.firmwareMemory = () => ({
  heap: py._module.HEAPU8.byteLength,
  histories: JSON.parse(py.runPython('json.dumps(dict(events=len(usb_runtime.board.event_log), interrupts=len(usb_runtime.board.interrupts.trace), operations=len(usb_runtime.host.operations), transfers=len(usb_runtime.control._endpoint.transfers)))'))
});`,
      });
    });
  }
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
  const worker = page.workers().find((value) => value.url().includes('emulator-worker'));
  assert.ok(worker);
  const inspect = () => worker.evaluate(() => self.firmwareMemory());
  const before = memoryCheck ? await inspect() : undefined;
  const repetitions = Number(process.env.FUJIMOD_CONSOLE_REPETITIONS ?? 1);
  assert.ok(Number.isSafeInteger(repetitions) && repetitions > 0);
  let completed = 0;
  for (let index = 0; index < repetitions; index++) {
    const previous = await page.locator('.is-console').filter({ hasText: 'Hello, world!' }).count();
    await page.getByLabel('Console command', { exact: true }).fill('hello');
    const started = Date.now();
    await page.getByRole('button', { name: 'Send ↵', exact: true }).click();
    await page.waitForFunction(
      (count) =>
        [...document.querySelectorAll('.is-console')].filter((entry) =>
          entry.textContent.includes('Hello, world!'),
        ).length > count || Boolean(document.querySelector('.level-error')),
      previous,
      { timeout: 15000 },
    );
    if (await page.locator('.level-error').count()) {
      assert.ok(memoryCheck, 'Unexpected USB failure');
      await page.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Connection interrupted',
      );
      assert.ok(Date.now() - started < 15000, 'A pending bus read must keep timeout timers live');
      assert.match(
        await page
          .locator('.level-error')
          .allTextContents()
          .then((values) => values.join(' ')),
        /tim(e|ed|eout)/i,
      );
      console.log(`Pending firmware reply timed out after ${completed} completed commands`);
      break;
    }
    completed++;
  }
  assert.ok(completed > 0, 'Must execute the real firmware console');
  if (memoryCheck) {
    const after = await inspect();
    assert.ok(
      after.heap <= before.heap + 8 * 1024 * 1024,
      `Heap grew across ${repetitions} commands: ${before.heap} → ${after.heap}`,
    );
    assert.ok(after.histories.events <= 4096);
    assert.ok(after.histories.interrupts <= 4096);
    assert.equal(after.histories.operations, 0);
    assert.equal(after.histories.transfers, 0);
    console.log(`Memory: ${JSON.stringify({ before, after, repetitions, completed })}`);
  }
  if (engine === webkit)
    assert.equal(usbWasmRequests.length, 1, 'Only the simulator needs USB Wasm');
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
