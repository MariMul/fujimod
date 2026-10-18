// Real served Dev Tools, ShareHost and WebRTC with synthetic WebUSB only.
import { chromium } from 'playwright';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { setTimeout } from 'node:timers/promises';
import assert from 'node:assert/strict';
if (!process.env.FUJIMOD_WEB_URL) throw Error('Set FUJIMOD_WEB_URL to the named HTTPS app URL');
const root = new URL('../../../../', import.meta.url).pathname;
const output = new URL('../../build/remote-traffic-browser/', import.meta.url);
await mkdir(output, { recursive: true });
const url = new URL('dev-tools/', process.env.FUJIMOD_WEB_URL);
const run = crypto.randomUUID();
url.searchParams.set('remote-traffic-test', run);
const upstream = await readFile(
  new URL('../../../../bindings/javascript/tests/webusb-fixture.mjs', import.meta.url),
  'utf8',
);
const consoleFixture = await readFile(new URL('./usb-fixture.mjs', import.meta.url), 'utf8');
const browser = await chromium.launch({
  headless: true,
  args: ['--disable-features=WebRtcHideLocalIpsWithMdns'],
});
try {
  const context = await browser.newContext({ viewport: { width: 1200, height: 900 } });
  await context.grantPermissions(['local-network-access'], { origin: url.origin });
  await context.addInitScript(
    upstream.replace('export function installWebUsb', 'function installWebUsb') +
      consoleFixture
        .replace(/^import .*;$/m, '')
        .replace('export function installConsoleFixture', 'function installConsoleFixture') +
      `
    const fixture = installConsoleFixture();
    fixture.iface.alternates[0].endpoints = [
      { endpointNumber: 2, direction: 'in', type: 'bulk', packetSize: 64 },
      { endpointNumber: 1, direction: 'out', type: 'bulk', packetSize: 64 },
    ];
    fixture.iface.alternate = fixture.iface.alternates[0];
    fixture.device.close = async () => { fixture.device.opened = false; fixture.iface.claimed = false; };
    navigator.usb.getDevices = async () => [fixture.device];
    const events = new EventTarget();
    navigator.usb.addEventListener = events.addEventListener.bind(events);
    navigator.usb.removeEventListener = events.removeEventListener.bind(events);
  `,
  );
  // Capture this isolated fixture's registration without advertising it to host tools.
  let link;
  await context.routeWebSocket(
    (url) => url.pathname === '/usb-proxy/dev-share',
    (socket) => {
      socket.onMessage((payload) => {
        link = JSON.parse(String(payload)).url;
        socket.send('{"type":"registered"}');
      });
    },
  );
  const owner = await context.newPage(),
    errors = [];
  owner.on('pageerror', (error) => {
    errors.push(String(error));
    console.error(String(error));
  });
  await owner.goto(url.href);
  await owner.getByRole('button', { name: 'Connect camera', exact: true }).click();
  await owner
    .getByText('Development share registered', { exact: true })
    .waitFor({ timeout: 30000 });
  assert.ok(link, 'The test page must create a standard share link');
  const remote = await context.newPage();
  await remote.goto(new URL('/dev-tools/', url).href);
  const result = await remote.evaluate(
    async ({ root, link }) => {
      const { connectPeer, parseShare } = await import(
        '/@fs/' + root + 'crates/usb-proxy/web/peer.js'
      );
      const { VERSION, encodeFrame, decodeFrame } = await import('/@fs/' + root + 'crates/usb-proxy/web/wire.js');
      const peer = await connectPeer(link);
      await peer.send(
        encodeFrame({
          type: 'hello',
          version: VERSION,
          role: 'controller',
          token: parseShare(link).key,
          reason: 'remote-traffic browser check',
        }),
      );
      const ready = decodeFrame(await peer.receive());
      if (ready.type !== 'ready') throw Error('Peer handshake failed');
      let id = 0,
        device;
      const request = async (operation) => {
        await peer.send(
          encodeFrame({ type: 'request', id: ++id, device, operation, timeout_ms: 5000 }),
        );
        const reply = decodeFrame(await peer.receive());
        if (reply.id !== id) throw Error('Wrong request correlation');
        return reply.completion;
      };
      const words = (...values) => {
        const data = new Uint8Array(values.length * 4),
          view = new DataView(data.buffer);
        values.forEach((n, i) => view.setUint32(i * 4, n, true));
        return data;
      };
      const control = (kind, size, offset = 0) => {
        const bytes = new Uint8Array(12),
          view = new DataView(bytes.buffer);
        bytes[0] = kind;
        view.setUint32(2, offset);
        view.setUint32(6, size);
        view.setUint16(10, kind === 0xc0 ? 0 : 0x1234);
        return request({
          op: 'control_out',
          setup: { request_type: 'vendor', recipient: 'other', request: 0, value: 0, index: 0 },
          data: bytes,
        });
      };
      const out = (bytes) => request({ op: 'transfer_out', endpoint: 1, data: bytes });
      const input = (length) => request({ op: 'transfer_in', endpoint: 0x82, length });
      const listed = await request({ op: 'list' });
      device = listed.devices[0].id;
      await request({ op: 'open', configuration: 1, interface: 2, alternate: 3 });
      await control(0xc0, 8);
      await input(64);
      const text = new TextEncoder().encode('hello\n'),
        command = new Uint8Array(28 + text.length);
      command.set(words(0x200001, 0, 0, 0, 6, text.length));
      command.set(text, 24);
      await control(0xd0, command.length);
      await out(command);
      await control(0xd1, 32);
      await input(64);
      const poll = words(0x200001, 0, 0, 0, 7, 4096, 0, 0);
      await control(0xd0, poll.length);
      await out(poll);
      await control(0xd1, 4096);
      await input(4096);
      for (let i = 0; i < 300; i++) {
        await control(0xe1, 64, i * 64);
        await input(64);
      }
      const failure = await request({ op: 'transfer_in', endpoint: 0x83, length: 64 });
      peer.close();
      return { requests: id, error: failure.error, controller: ready.id };
    },
    { root, link },
  );
  assert.match(result.error, /Endpoint unavailable/);
  await owner.locator('.is-console').filter({ hasText: 'Fuji console ready' }).waitFor();
  await owner.getByText('Console submit', { exact: true }).first().waitFor();
  await owner.getByText('Console poll', { exact: true }).first().waitFor();
  await owner.locator('.dt-message').filter({ hasText: 'Endpoint unavailable' }).first().waitFor();
  await setTimeout(350);
  const summaries = await owner
    .locator('.dt-message')
    .filter({ hasText: 'transfer_in ep 0x82' })
    .count();
  assert.ok(summaries < 100, `300 data chunks must be summarized, saw ${summaries} rows`);
  assert.ok((await owner.locator('.dt-workspace').innerText()).includes(result.controller));
  assert.deepEqual(errors, []);
  await owner.screenshot({ path: new URL('owner.png', output).pathname, fullPage: true });
  await writeFile(
    new URL('result.json', output),
    JSON.stringify({ passed: true, ...result, summaries, url: url.href }, null, 2),
  );
  await owner.getByRole('button', { name: 'Share', exact: true }).click();
  await owner.getByRole('button', { name: 'Stop sharing', exact: true }).click();
  console.log(
    `PASS remote FF80 and console traffic, ${result.requests} requests, ${summaries} bulk summary rows and visible USB errors`,
  );
} catch (error) {
  for (const [i, page] of browser
    .contexts()
    .flatMap((context) => context.pages())
    .entries()) {
    await writeFile(
      new URL(`failure-${i}.txt`, output),
      await page.locator('body').innerText(),
    ).catch(() => {});
    await page.screenshot({ path: new URL(`failure-${i}.png`, output).pathname }).catch(() => {});
  }
  throw error;
} finally {
  await browser.close();
}
