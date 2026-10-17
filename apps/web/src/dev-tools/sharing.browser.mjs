// Real stock-firmware simulator -> WebRTC -> browser and native FujiMod clients.
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL to the named development HTTPS URL');
const output = new URL('../../build/sharing-browser/', import.meta.url);
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const errors = [];
try {
  for (const controller of ['browser', 'native']) {
    const context = await browser.newContext({ viewport: { width: 1100, height: 800 } });
    await context.grantPermissions(['clipboard-read', 'clipboard-write', 'local-network-access'], {
      origin: new URL(base).origin,
    });
    const owner = await context.newPage();
    await context.routeWebSocket(
      (url) => url.searchParams.has('token'),
      (socket) => socket.close(),
    );
    owner.on('requestfailed', (request) =>
      console.error(
        'Owner fetch failed:',
        new URL(request.url()).pathname,
        request.failure()?.errorText,
      ),
    );
    owner.on('pageerror', (e) => {
      errors.push(String(e));
      console.error('Owner error:', String(e));
    });
    const url = new URL('dev-tools/', base);
    url.searchParams.set('target', 'simulator');
    url.searchParams.set('firmware', 'x70-110');
    url.searchParams.set('connect', '1');
    await owner.addInitScript(() => {
      if ('usb' in navigator)
        Object.defineProperty(navigator, 'usb', {
          configurable: true,
          value: Object.assign(new EventTarget(), {
            getDevices() {
              throw Error('Physical USB forbidden');
            },
            requestDevice() {
              throw Error('Physical USB forbidden');
            },
          }),
        });
    });
    await owner.goto(url.href);
    await owner.waitForFunction(
      () =>
        document.querySelector('.dt-status')?.textContent?.trim() === 'Connected' ||
        document.querySelector('.level-error'),
      null,
      { timeout: 240000 },
    );
    assert.equal(
      (await owner.locator('.dt-status').textContent()).trim(),
      'Connected',
      await owner.locator('.dt-workspace').innerText(),
    );
    console.log('Stock simulator connected');
    await owner.getByRole('button', { name: 'Share', exact: true }).click();
    const warning = owner.getByText(
      'Anyone with a share link will be able to control your USB device.',
      { exact: true },
    );
    assert.equal(await warning.evaluate((e) => e.tagName), 'STRONG');
    assert.ok(Number(await warning.evaluate((e) => getComputedStyle(e).fontWeight)) >= 700);
    assert.equal(await warning.evaluate((e) => getComputedStyle(e).color), 'rgb(255, 73, 73)');
    await owner.screenshot({ path: new URL('share-popup.png', output).pathname });
    await owner.locator('.dt-share-camera').click();
    await owner.getByText('Share link copied', { exact: true }).waitFor({ timeout: 20000 });
    const link = await owner.evaluate(() => navigator.clipboard.readText());
    assert.ok(link.startsWith(url.href + '#?remotecontrol='));
    assert.equal((await owner.locator('.dt-status').textContent()).trim(), 'Sharing');
    assert.equal(await owner.getByLabel('Console command', { exact: true }).isDisabled(), true);
    let native;
    if (controller === 'native') {
      const python = new URL('../../../../../.venv/bin/python', import.meta.url).pathname;
      const script = String.raw`
import json,os
from fujimod import usb
from fujimod.usb import blocking
assert 'FUJIMOD_USB_PROXY_TOKEN' not in os.environ
devices=blocking.discover(usb.DeviceFilter.Diagnostic)
assert len(devices)==1
choices=blocking.interfaces(devices[0])
limits=usb.FirmwareLimits(memory_read=256,memory_write=256,configuration=256,console=4096)
trace=usb.tracing.Trace(128,65536)
session=blocking.Session.open(devices[0],choices[0],limits,timeout_ms=10000,trace=trace)
session.submit_console(b'hello\n')
import time
output=b''
for _ in range(100):
 reply=session.poll_console(4096)
 output+=reply.output
 if reply.progress==usb.ConsoleProgress.Complete: break
 assert reply.progress==usb.ConsoleProgress.Pending
 time.sleep(.05)
assert reply.progress==usb.ConsoleProgress.Complete
assert b'Hello, world!' in output,output
session.close()
print(json.dumps({'native':True}))
`;
      const env = { ...process.env, FUJIMOD_USB_PROXY_URL: link };
      delete env.FUJIMOD_USB_PROXY_TOKEN;
      const child = spawn(python, ['-c', script], { env, stdio: ['ignore', 'pipe', 'pipe'] });
      let stdout = '',
        stderr = '';
      child.stdout.on('data', (data) => {
        stdout += data;
      });
      child.stderr.on('data', (data) => {
        stderr += data;
      });
      const exit = await new Promise((resolve, reject) => {
        child.on('exit', resolve);
        child.on('error', reject);
      });
      assert.equal(exit, 0, stderr);
      native = JSON.parse(stdout);
      assert.equal(native.native, true);
      console.log('Native Python used the full share URL as its only USB environment setting');
    }
    const receiverContext = await browser.newContext();
    await receiverContext.routeWebSocket(
      (url) => url.searchParams.has('token'),
      (socket) => socket.close(),
    );
    await receiverContext.grantPermissions(['local-network-access'], {
      origin: new URL(base).origin,
    });
    if (controller === 'browser') {
      const receiver = await receiverContext.newPage();
      receiver.on('pageerror', (e) => {
        errors.push(String(e));
        console.error('Receiver error:', String(e));
      });
      await receiver.goto(link);
      await receiver.waitForFunction(
        () =>
          document.querySelector('.dt-status')?.textContent?.trim() === 'Connected' ||
          document.querySelector('.level-error'),
        null,
        { timeout: 60000 },
      );
      assert.equal(
        (await receiver.locator('.dt-status').textContent()).trim(),
        'Connected',
        await receiver.locator('.dt-workspace').innerText(),
      );
      await receiver.getByLabel('Console command', { exact: true }).fill('hello');
      await receiver.locator('.dt-composer button[type="submit"]').click();
      await receiver
        .locator('.is-console')
        .filter({ hasText: 'Hello, world!' })
        .waitFor({ timeout: 60000 });
      await owner
        .locator('.dt-message')
        .filter({ hasText: 'Remote controller connected' })
        .first()
        .waitFor();
      console.log('Browser opened shared URL and executed stock console hello');
      await receiver.screenshot({ path: new URL('remote-controller.png', output).pathname });
      await receiver.getByLabel('Console command', { exact: true }).fill('hello');
      await receiver.waitForFunction(
        () => !document.querySelector('.dt-composer button[type=submit]').disabled,
        null,
        { timeout: 20000 },
      );
      await receiver.getByLabel('Console command', { exact: true }).fill('');
      await receiver.getByRole('button', { name: 'Disconnect', exact: true }).click();
      await receiver.waitForFunction(
        () => document.querySelector('.dt-status')?.textContent?.trim() === 'Disconnected',
      );
    }
    await owner.getByRole('button', { name: 'Share', exact: true }).click();
    await owner.getByRole('button', { name: 'Stop sharing', exact: true }).click();
    const expired = await receiverContext.newPage();
    await expired.goto(link);
    await expired.locator('.level-error').first().waitFor({ timeout: 35000 });
    assert.notEqual((await expired.locator('.dt-status').textContent()).trim(), 'Connected');
    assert.deepEqual(errors, []);
    await writeFile(
      new URL(`result-${controller}.json`, output),
      JSON.stringify({ passed: true, native, hello: true, revoked: true }, null, 2),
    );
    console.log(
      `PASS ${controller}: real stock simulator, warning, clipboard, remote hello, connection logs and revocation`,
    );
    await context.close();
    await receiverContext.close();
  }
} catch (error) {
  for (const [i, context] of browser.contexts().entries())
    for (const [j, page] of context.pages().entries()) {
      await writeFile(
        new URL(`failure-${i}-${j}.txt`, output),
        await page.locator('body').innerText(),
      ).catch(() => {});
      await page
        .screenshot({ path: new URL(`failure-${i}-${j}.png`, output).pathname })
        .catch(() => {});
    }
  throw error;
} finally {
  await browser.close();
}
