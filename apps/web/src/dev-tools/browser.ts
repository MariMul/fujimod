import { browserScript } from '../../scripts/browser-script.ts';
declare global {
  interface Window {
    consoleFixture: ReturnType<typeof import('./usb-fixture.ts').installConsoleFixture>;
  }
}
import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
let base = process.env.FUJIMOD_WEB_URL;
let server;
try {
  if (!base) {
    base = 'http://127.0.0.1:5199/';
    const web = new URL('../../', import.meta.url);
    server = spawn(
      process.execPath,
      [
        fileURLToPath(new URL('node_modules/vite-plus/bin/vp', web)),
        'preview',
        '--host',
        '127.0.0.1',
        '--port',
        '5199',
        '--strictPort',
      ],
      { cwd: fileURLToPath(web), stdio: 'pipe', detached: process.platform !== 'win32' },
    );
    let output = '';
    server.stdout.on('data', (value) => {
      output += value;
    });
    server.stderr.on('data', (value) => {
      output += value;
    });
    const deadline = Date.now() + 15000;
    let ready = false;
    while (Date.now() < deadline && server.exitCode === null) {
      try {
        ready = (await fetch(base)).ok;
      } catch {
        /* Server is starting. */
      }
      if (ready) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (!ready) throw Error(`Preview did not start: ${output}`);
  }
  const url = new URL('dev-tools/', base).href;
  const directory = new URL('../../build/dev-tools-browser/', import.meta.url);
  await mkdir(directory, { recursive: true });
  const fixture = await browserScript(new URL('./usb-fixture.ts', import.meta.url));
  const upstream = await browserScript(
    new URL('../../../../bindings/javascript/tests/webusb-fixture.ts', import.meta.url),
  );
  for (const mobile of [false, true]) {
    const browser = await chromium.launch({ headless: true });
    try {
      const context = await browser.newContext({
        viewport: mobile ? { width: 390, height: 844 } : { width: 1300, height: 900 },
        isMobile: mobile,
        hasTouch: mobile,
      });
      await context.grantPermissions(
        ['local-network-access', 'clipboard-read', 'clipboard-write'],
        { origin: new URL(url).origin },
      );
      const page = await context.newPage();
      page.setDefaultTimeout(15000);
      const errors: string[] = [];
      page.on('pageerror', (error) => errors.push(error.message));
      await page.route('**/__test__/upstream.mjs', (route) =>
        route.fulfill({ contentType: 'text/javascript', body: upstream }),
      );
      await page.route('**/__test__/console.mjs', (route) =>
        route.fulfill({
          contentType: 'text/javascript',
          body: fixture.replace(
            '../../../../bindings/javascript/tests/webusb-fixture.ts',
            './upstream.mjs',
          ),
        }),
      );
      await page.goto(url);
      await page.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Disconnected',
      );
      await page.evaluate(async () => {
        window.consoleFixture = (
          (await import('/__test__/' + 'console.mjs')) as typeof import('./usb-fixture.ts')
        ).installConsoleFixture();
      });
      await page
        .locator('.td-connects')
        .getByRole('button', { name: 'Connect camera', exact: true })
        .click();
      await page.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Sharing',
        null,
        { timeout: 10000 },
      );
      assert.equal(
        await page.evaluate(
          () => window.consoleFixture.log.filter((entry) => entry[0] === 'picker').length,
        ),
        1,
      );
      console.log('Shared camera ready, sending console command');
      await page.getByLabel('Console command', { exact: true }).fill('help');
      await page.getByRole('button', { name: 'Send ↵', exact: true }).click();
      await page
        .getByText('Fuji console ready', { exact: false })
        .first()
        .waitFor({ timeout: 10000 })
        .catch(async (error) => {
          console.error(
            await page.locator('body').innerText(),
            await page.evaluate(() => window.consoleFixture.log),
          );
          throw error;
        });
      const capture = await page.evaluate(() => {
        const log = window.consoleFixture.log;
        const sent = log.filter(
          (e) =>
            e[0] === 'out' && new DataView((e[2] as Uint8Array).buffer).getUint32(16, true) === 6,
        );
        const viewport = document.querySelector('.dt-viewport')!;
        return {
          sent: Array.from((sent[0][2] as Uint8Array).subarray(24, -4)),
          frames: log.filter((e) => e[0] === 'control').length,
          overflow: document.documentElement.scrollWidth > innerWidth,
          scrollable: viewport.scrollWidth > viewport.clientWidth,
          whiteSpace: getComputedStyle(document.querySelector('.is-console .dt-message')!)
            .whiteSpace,
          logTop: viewport.getBoundingClientRect().top,
          logFraction: viewport.clientHeight / innerHeight,
          rowHeight: document.querySelector('.dt-row')!.getBoundingClientRect().height,
          pageHeight: document.documentElement.scrollHeight,
          screenHeight: innerHeight,
          panes: document.querySelectorAll('[aria-label="Connection log"]').length,
        };
      });
      assert.deepEqual(capture.sent, [...new TextEncoder().encode('help\n')]);
      assert.equal(capture.overflow, false);
      if (mobile) assert.equal(capture.scrollable, true);
      assert.equal(capture.whiteSpace, 'pre');
      assert.equal(capture.panes, 1);
      if (!mobile) assert.ok(capture.logTop < 100, 'log begins directly below compact controls');
      assert.ok(capture.logFraction > (mobile ? 0.35 : 0.75), 'log gets most of the viewport');
      assert.ok(capture.rowHeight <= 22, 'single-line log rows remain dense');
      if (!mobile)
        assert.ok(capture.pageHeight <= capture.screenHeight, 'composer stays on screen');
      // Collapsed messages are capped at 120ch; expand the actual long response
      // before requiring the log viewport to scroll horizontally on desktop.
      const longResponse = page
        .locator('.is-console .dt-row')
        .filter({ hasText: '0123456789abcdef' })
        .first();
      await longResponse.click();
      await page.locator('.is-console .dt-detail pre').first().waitFor();
      const expectedOutput = new TextEncoder().encode(
        'Fuji console ready\n' + '0123456789abcdef '.repeat(28) + '\n',
      );
      assert.equal(
        await page.locator('.is-console .dt-detail pre').first().textContent(),
        Array.from(expectedOutput, (byte) => byte.toString(16).padStart(2, '0')).join(' '),
      );
      assert.equal(
        await page
          .locator('.dt-viewport')
          .evaluate((viewport) => viewport.scrollWidth > viewport.clientWidth),
        true,
      );
      await longResponse.click();
      await page.getByRole('button', { name: 'USB', exact: true }).click();
      await page.getByText('Control Open submitted', { exact: false }).first().waitFor();
      await page.screenshot({
        path: new URL(mobile ? 'mobile.png' : 'desktop.png', directory).pathname,
        fullPage: true,
      });
      // Every local read uses a new ff80 session while the share stays online.
      for (const name of ['Read device identity', 'Dump configuration', 'Read memory']) {
        await page.getByRole('button', { name, exact: true }).click();
        await page.waitForFunction(
          () => document.querySelector('.dt-reads [role="status"]')?.textContent !== 'Reading…',
        );
        assert.equal(await page.locator('.dt-reads [role="status"]').innerText(), 'Read complete');
        assert.equal(await page.locator('.dt-status').innerText(), 'Sharing');
      }
      console.log('Shared local reads complete');
      const downloadEvent = page.waitForEvent('download');
      await page.getByRole('button', { name: 'Download diagnostic bundle', exact: true }).click();
      const path = await (await downloadEvent).path();
      assert.ok(path);
      const bundle = JSON.parse(await readFile(path, 'utf8'));
      assert.equal(bundle.readings.length, 25);
      assert.equal(bundle.readings.at(-1).data.length, 256);
      await page.getByRole('button', { name: 'Sharing details', exact: true }).click();
      await page.locator('.dt-share-camera').click();
      const link = await page.evaluate(() => navigator.clipboard.readText());
      if (await page.getByRole('button', { name: 'Close share dialog', exact: true }).isVisible())
        await page.getByRole('button', { name: 'Close share dialog', exact: true }).click();
      const remoteContext = await browser.newContext();
      await remoteContext.grantPermissions(['local-network-access'], {
        origin: new URL(url).origin,
      });
      await remoteContext.addInitScript(() => {
        Object.defineProperty(navigator, 'usb', {
          configurable: true,
          value: Object.assign(new EventTarget(), {
            getDevices() {
              throw Error('Physical USB forbidden in remote test');
            },
            requestDevice() {
              throw Error('Physical USB forbidden in remote test');
            },
          }),
        });
      });
      const remote = await remoteContext.newPage();
      await remote.goto(link);
      await remote.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Connected',
        null,
        { timeout: 60000 },
      );
      console.log('Remote controller claimed the shared camera');
      const transfers = await page.evaluate(() => window.consoleFixture.log.length);
      await page.getByRole('button', { name: 'Read memory', exact: true }).click();
      await page.waitForFunction(() =>
        document
          .querySelector('.dt-reads [role="status"]')
          ?.textContent?.includes('in use by a remote controller'),
      );
      await page.getByLabel('Console command', { exact: true }).fill('help');
      await page.getByLabel('Console command', { exact: true }).press('Enter');
      await page
        .locator('.level-error')
        .filter({ hasText: 'in use by a remote controller' })
        .first()
        .waitFor();
      assert.equal(await page.evaluate(() => window.consoleFixture.log.length), transfers);
      await remote.getByRole('button', { name: 'Disconnect', exact: true }).click();
      await remote.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Disconnected',
      );
      await page.getByRole('button', { name: 'Read memory', exact: true }).click();
      await page.waitForFunction(
        () => document.querySelector('.dt-reads [role="status"]')?.textContent === 'Read complete',
      );
      await remoteContext.close();
      await page.screenshot({
        path: new URL(mobile ? 'shared-mobile.png' : 'shared-desktop.png', directory).pathname,
        fullPage: true,
      });
      await page.getByRole('button', { name: 'Disconnect', exact: true }).click();
      // This command completed normally: the existing controller closes a clean
      // session. Its interrupted-operation path abandons host ownership instead.
      await page.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Disconnected',
      );
      assert.equal(
        await page.evaluate(
          () => window.consoleFixture.log.filter((e) => e[0] === 'control' && e[1] === 0xc4).length,
        ),
        7,
      );
      assert.deepEqual(errors, []);
      console.log(
        `PASS ${mobile ? 'mobile' : 'desktop'}: production WebUSB console, layered logs, horizontal scroll, clean session disconnect`,
      );
    } finally {
      await browser.close();
    }
  }
  const browser = await webkit.launch({ headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
    await page.goto(url);
    await page.getByText('WebUSB unavailable', { exact: true }).waitFor();
    // Without a connection there is no Console tool, so command entry is absent
    // rather than present-but-disabled.
    assert.equal(await page.getByLabel('Console command', { exact: true }).count(), 0);
    console.log('PASS WebKit: unsupported WebUSB is visible and command entry is unavailable');
  } finally {
    await browser.close();
  }
} finally {
  if (server) {
    if (process.platform === 'win32') server.kill();
    else if (server.pid !== undefined) {
      try {
        process.kill(-server.pid, 'SIGTERM');
      } catch (error) {
        if (!(error instanceof Error && 'code' in error && error.code === 'ESRCH')) {
          console.error('Could not stop browser test preview:', error);
          process.exitCode = 1;
        }
      }
    }
    server.stdout.destroy();
    server.stderr.destroy();
  }
}
