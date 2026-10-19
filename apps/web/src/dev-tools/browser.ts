import { browserScript } from '../../scripts/browser-script.ts';
declare global {
  interface Window {
    consoleFixture: ReturnType<typeof import('./usb-fixture.ts').installConsoleFixture>;
  }
}
import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
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
      const page = await context.newPage();
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
      await page.getByRole('button', { name: 'Connect camera', exact: true }).click();
      await page.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent === 'Connected',
      );
      await page.getByLabel('Console command', { exact: true }).fill('help');
      await page.getByRole('button', { name: 'Send ↵', exact: true }).click();
      await page.getByText('Fuji console ready', { exact: false }).first().waitFor();
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
      assert.equal(capture.scrollable, true);
      assert.equal(capture.whiteSpace, 'pre');
      assert.equal(capture.panes, 1);
      assert.ok(capture.logTop < 100, 'log begins directly below compact controls');
      assert.ok(capture.logFraction > 0.75, 'log gets most of the viewport');
      assert.ok(capture.rowHeight <= 22, 'single-line log rows remain dense');
      assert.ok(capture.pageHeight <= capture.screenHeight, 'composer stays on screen');
      await page.getByRole('button', { name: 'USB', exact: true }).click();
      await page.getByText('Control Open submitted', { exact: false }).first().waitFor();
      await page.screenshot({
        path: new URL(mobile ? 'mobile.png' : 'desktop.png', directory).pathname,
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
        1,
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
    assert.equal(await page.getByLabel('Console command', { exact: true }).isDisabled(), true);
    console.log('PASS WebKit: unsupported WebUSB is visible and command entry is disabled');
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
