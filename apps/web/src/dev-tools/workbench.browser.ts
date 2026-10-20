// Simulator-only integration and screenshots against the configured web preview.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the configured development server.');
const directory = resolve(process.env.FUJIMOD_SCREENSHOTS ?? 'build/workbench');
await mkdir(directory, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  for (const phone of [false, true]) {
    const context = await browser.newContext({
      viewport: phone ? { width: 390, height: 844 } : { width: 1440, height: 1000 },
      reducedMotion: 'reduce',
      isMobile: phone,
      hasTouch: phone,
    });
    await context.addInitScript(() => {
      Object.defineProperty(navigator, 'usb', {
        configurable: true,
        value: Object.assign(new EventTarget(), {
          getDevices() {
            throw Error('Physical USB forbidden in workbench test');
          },
          requestDevice() {
            throw Error('Physical USB forbidden in workbench test');
          },
        }),
      });
    });
    const page = await context.newPage();
    const errors: string[] = [];
    page.on('pageerror', (error) => {
      errors.push(error.message);
      console.error(error.message);
    });
    const size = phone ? 'phone' : 'desktop';
    await page.goto(new URL('dev-tools/', base).href);
    await page.waitForFunction(
      () => document.querySelector('.dt-status')?.textContent === 'Disconnected',
    );
    await page.getByRole('heading', { name: 'Enter service mode', exact: true }).waitFor();
    await page.screenshot({ path: resolve(directory, `deck-${size}.png`), fullPage: true });
    await page.getByRole('button', { name: 'Show instructions', exact: true }).click();
    await page.getByRole('button', { name: 'Begin', exact: true }).click();
    await page.getByText('In service mode, do not use the OFF switch.', { exact: true }).waitFor();
    assert.ok((await page.locator('.dm-entry').innerText()).includes('Hold UP'));
    assert.ok((await page.locator('.dm-warning').innerText()).includes('remove the battery'));
    const url = new URL('dev-tools/', base);
    url.searchParams.set('target', 'simulator');
    url.searchParams.set('firmware', process.env.FUJIMOD_FIRMWARE ?? 'x70-110');
    url.searchParams.set('connect', '1');
    console.log(`${size}: booting simulator`);
    await page.goto(url.href);
    await page.waitForFunction(
      () =>
        document.querySelector('.dt-status')?.textContent === 'Connected' ||
        Boolean(document.querySelector('.level-error')),
      null,
      { timeout: 240000 },
    );
    assert.equal(
      await page.locator('.dt-status').innerText(),
      'Connected',
      await page.locator('.dt-workspace').innerText(),
    );
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    await page.locator('.dt-share-camera').click();
    await page.waitForFunction(
      () => document.querySelector('.dt-status')?.textContent === 'Sharing',
    );
    if (await page.getByRole('button', { name: 'Close share dialog', exact: true }).isVisible())
      await page.getByRole('button', { name: 'Close share dialog', exact: true }).click();
    await page.waitForFunction(
      () => !document.querySelector<HTMLInputElement>('[aria-label="Console command"]')!.disabled,
    );
    assert.equal(await page.getByLabel('Console command', { exact: true }).isEnabled(), true);
    console.log(`${size}: simulator shared, reading identity locally`);
    await page.getByRole('button', { name: 'Read device identity', exact: true }).click();
    await page.waitForFunction(
      () => document.querySelector('.dt-reads [role="status"]')?.textContent !== 'Reading…',
      null,
      { timeout: 30000 },
    );
    assert.equal(await page.locator('.dt-reads [role="status"]').innerText(), 'Read complete');
    const geometry = await page.evaluate(() => {
      const tools = document.querySelector('.dt-tool-column')!.getBoundingClientRect();
      const log = document.querySelector('.dt-console')!.getBoundingClientRect();
      const launchersFit = [...document.querySelectorAll('.dt-tool-launchers > *')].every(
        (element) => {
          const rect = element.getBoundingClientRect();
          return rect.x >= tools.x && rect.right <= tools.right;
        },
      );
      return {
        launchersFit,
        tools: { x: tools.x, width: tools.width, bottom: tools.bottom },
        log: { x: log.x, y: log.y },
        overflow: document.documentElement.scrollWidth > innerWidth,
      };
    });
    assert.equal(geometry.overflow, false);
    assert.equal(geometry.launchersFit, true, 'Every launcher fits inside the tool column');
    if (phone) assert.ok(geometry.log.y >= geometry.tools.bottom);
    else {
      assert.ok(geometry.tools.width >= 320 && geometry.tools.width <= 380);
      assert.ok(geometry.log.x > geometry.tools.x + geometry.tools.width);
    }
    await page.locator('.dt-viewport').evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });
    await page.screenshot({ path: resolve(directory, `workbench-${size}.png`), fullPage: true });
    for (const name of ['Console', 'Enter debug mode', 'Leave debug mode']) {
      await page
        .getByRole('navigation', { name: 'Workbench tools' })
        .getByRole('button', { name, exact: true })
        .click();
      assert.equal(await page.getByRole('region', { name: 'Connection log' }).isVisible(), true);
      // The console command input belongs to the Console tool; other tools show a
      // status line naming the current tool, keeping the bar the same height.
      const isConsole = name === 'Console';
      assert.equal(
        await page.getByLabel('Console command', { exact: true }).isVisible(),
        isConsole,
      );
      if (!isConsole) assert.equal(await page.locator('.dt-bartool').innerText(), name);
    }
    // The camera reads are always available while connected — no tool selection.
    await page
      .getByRole('navigation', { name: 'Workbench tools' })
      .getByRole('button', { name: 'Console', exact: true })
      .click();
    assert.equal(await page.getByRole('region', { name: 'Connection log' }).isVisible(), true);
    await page.getByLabel('Address', { exact: true }).fill('0x008a0400');
    await page.getByLabel('Length (bytes)', { exact: true }).fill('32');
    await page.getByRole('button', { name: 'Read memory', exact: true }).click();
    await page.waitForFunction(
      () => document.querySelector('.dt-reads [role="status"]')?.textContent !== 'Reading…',
    );
    const memoryStatus = await page.locator('.dt-reads [role="status"]').innerText();
    // A stock simulator can refuse debug-memory access. Verify either accepted
    // bytes or its real failed transfer and blocked session; never substitute data.
    if (memoryStatus !== 'Read complete') {
      assert.match(memoryStatus, /USB transfer failed during (BulkIn|Control\(WriteCommand\))/);
      assert.equal(await page.locator('.dt-status').innerText(), 'Connection interrupted');
      assert.equal(
        await page.getByRole('button', { name: 'Read memory', exact: true }).isDisabled(),
        true,
      );
      console.log(`${size}: simulator refused memory read; failure retained`);
    }
    const problem = page.getByRole('dialog', { name: 'Camera problem', exact: true });
    if (await problem.isVisible()) {
      await page.keyboard.press('Escape');
      await problem.waitFor({ state: 'hidden' });
    }
    const downloadEvent = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Download diagnostic bundle', exact: true }).click();
    const download = await downloadEvent;
    const path = await download.path();
    assert.ok(path);
    const bundle = JSON.parse(await readFile(path, 'utf8'));
    assert.equal(bundle.readings.length, 8);
    assert.ok(
      bundle.readings
        .slice(0, 7)
        .every(
          (reading: { error?: string; data: number[] }) =>
            !reading.error && reading.data.length > 0,
        ),
    );
    if (memoryStatus === 'Read complete') assert.equal(bundle.readings.at(-1).data.length, 32);
    else {
      assert.match(bundle.readings.at(-1).error, /USB transfer failed/);
      assert.equal(bundle.readings.at(-1).data, undefined);
    }
    assert.deepEqual(errors, []);
    console.log(
      `PASS ${size}: service guide, simulator ff80 identity + memory, JSON bundle, responsive workbench and persistent log`,
    );
    await context.close();
  }
} finally {
  await browser.close();
}
