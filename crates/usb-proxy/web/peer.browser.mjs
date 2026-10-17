// Synthetic USB over real browser<->Rust WebRTC, including SCTP chunking.
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const require = createRequire(
  new URL('../../../bindings/javascript/package.json', import.meta.url),
);
const { chromium } = require('playwright');
const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL');
const repo = new URL('../../../', import.meta.url).pathname;
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext();
  await context.grantPermissions(['local-network-access'], { origin: new URL(base).origin });
  await context.routeWebSocket(
    (url) => url.searchParams.has('token'),
    (socket) => socket.close(),
  );
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(new URL('usb-proxy/', base).href);
  const link = await page.evaluate(
    async ({ repo, base }) => {
      const { installWebUsb } = await import(
        '/@fs/' + repo + 'bindings/javascript/tests/webusb-fixture.mjs'
      );
      const fixture = installWebUsb();
      fixture.iface.alternates[0].endpoints = [
        { endpointNumber: 2, direction: 'in', type: 'bulk', packetSize: 64 },
        { endpointNumber: 1, direction: 'out', type: 'bulk', packetSize: 64 },
      ];
      fixture.iface.alternate = fixture.iface.alternates[0];
      fixture.device.close = async () => {
        fixture.device.opened = false;
        fixture.iface.claimed = false;
      };
      const { exportUsbDevice } = await import(
        '/@fs/' + repo + 'apps/web/src/dev-tools/usb-sharing.ts'
      );
      const { ShareHost, newTicket, shareUrl } = await import(
        '/@fs/' + repo + 'crates/usb-proxy/web/peer.js'
      );
      const signal = new URL('usb-proxy/signal', base);
      signal.protocol = 'wss:';
      const ticket = newTicket(signal.href, []),
        logs = [];
      const host = await ShareHost.create(ticket, await exportUsbDevice(fixture.device), {
        connected: () => logs.push('connected'),
        disconnected: () => logs.push('disconnected'),
        error: (e) => logs.push(e.message),
      });
      const link = shareUrl(new URL('dev-tools/', base).href, ticket);
      globalThis.peerTest = { host, logs, fixture, link };
      return link;
    },
    { repo, base },
  );
  // Probe the real peer transport without claiming or accessing USB.
  const echo = await page.evaluate(async (repo) => {
    const { connectPeer, parseShare } = await import('/@fs/' + repo + 'crates/usb-proxy/web/peer.js');
    const before = JSON.stringify(peerTest.fixture.log);
    const peer = await connectPeer(peerTest.link);
    try {
      await peer.send(JSON.stringify({ type: 'hello', version: 1, role: 'controller',
        token: parseShare(peerTest.link).key }));
      const ready = JSON.parse(await peer.receive());
      if (ready.type !== 'ready') throw Error('Peer handshake failed');
      await peer.send(JSON.stringify({ type: 'request', id: 1, device: '',
        operation: { op: 'probe' }, timeout_ms: 10 }));
      const reply = JSON.parse(await peer.receive());
      return { reply, untouched: before === JSON.stringify(peerTest.fixture.log) };
    } finally { peer.close(); }
  }, repo);
  assert.equal(echo.untouched, true);
  assert.deepEqual(echo.reply, { type: 'reply', id: 1,
    completion: { devices: [], data: '', transferred: 0, error: null } });
  console.log('PASS USB-free latency echo over real WebRTC');
  const python = new URL('../../../../.venv/bin/python', import.meta.url).pathname;
  async function native(script) {
    const env = { ...process.env, FUJIMOD_USB_PROXY_URL: link };
    delete env.FUJIMOD_USB_PROXY_TOKEN;
    const child = spawn(python, ['-c', script], { env, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '',
      err = '';
    child.stdout.on('data', (d) => (out += d));
    child.stderr.on('data', (d) => (err += d));
    const exit = await new Promise((resolve, reject) => {
      child.on('exit', resolve);
      child.on('error', reject);
    });
    assert.equal(exit, 0, err);
    return out;
  }
  const setup = String.raw`
from fujimod import usb
from fujimod.usb import blocking
devices=blocking.discover(usb.DeviceFilter.Diagnostic)
assert len(devices)==1
choices=blocking.interfaces(devices[0])
limits=usb.FirmwareLimits(memory_read=256,memory_write=256,configuration=256,console=4096)
`;
  await native(
    setup +
      String.raw`
session=blocking.Session.open(devices[0],choices[0],limits,timeout_ms=20000)
assert session.device_info(usb.InfoSelector.Product)=='SYNTHETIC'
data=bytes(range(256))*8192
session.write_data(data)
assert session.read_data(0,len(data))==data
session.close()
print('native 2 MiB upload and download passed')
`,
  );
  assert.equal(await page.evaluate(() => peerTest.fixture.state.pieces[0].length), 2 * 1024 * 1024);
  console.log('PASS native WebRTC 2 MiB USB payloads in both directions');
  const pythonFiles = execFileSync('rg', ['--files', 'bindings/python/python', '-g', '*.py'], {
    cwd: repo,
    encoding: 'utf8',
  })
    .trim()
    .split('\n');
  const paths = Object.fromEntries(
    await Promise.all(
      pythonFiles.map(async (path) => [
        path.slice('bindings/python/python/'.length),
        await readFile(repo + path, 'utf8'),
      ]),
    ),
  );
  await page.evaluate(
    async ({ repo, paths }) => {
      const pyRoot = '/@fs/' + repo + 'bindings/javascript/node_modules/pyodide/';
      const { loadPyodide } = await import(pyRoot + 'pyodide.mjs');
      const { registerPyodide, initialize } = await import(
        '/@fs/' + repo + 'bindings/javascript/dist/index.js'
      );
      const py = await loadPyodide({ indexURL: pyRoot });
      await initialize();
      registerPyodide(py);
      for (const [path, source] of Object.entries(paths)) {
        py.FS.mkdirTree('/proxy-python/' + path.slice(0, path.lastIndexOf('/')));
        py.FS.writeFile('/proxy-python/' + path, source);
      }
      py.globals.set('share_url', peerTest.link);
      await py.runPythonAsync(`
import sys
sys.path.insert(0,'/proxy-python')
from fujimod import usb
devices=await usb.discover_proxy(share_url,'',usb.DeviceFilter.Diagnostic)
choices=await usb.interfaces(devices[0])
limits=usb.FirmwareLimits(memory_read=256,memory_write=256,configuration=256,console=4096)
session=await usb.Session.open(devices[0],choices[0],limits)
assert await session.device_info(usb.InfoSelector.Product)=='SYNTHETIC'
assert await session.read_data(123,5)==bytes([123,124,125,126,127])
await session.close()
`);
    },
    { repo, paths },
  );
  console.log('PASS Pyodide controller using the same share URL');
  await page.evaluate(async (repo) => {
    const { initialize, usb } = await import('/@fs/' + repo + 'bindings/javascript/dist/index.js');
    await initialize();
    const devices = await usb.discoverProxy(peerTest.link, '', usb.DeviceFilter.Diagnostic);
    const choices = await devices[0].interfaces();
    const session = await usb.Session.open(
      devices[0],
      choices[0],
      new usb.FirmwareLimits(256, 256, 256, 4096),
      new usb.TransferOptions(5000, 8388608),
    );
    peerTest.session = session;
  }, repo);
  await native(
    setup +
      String.raw`
try:
 blocking.Session.open(devices[0],choices[0],limits)
except usb.UsbError as error:
 assert 'in use' in str(error),str(error)
else:
 raise AssertionError('second controller claimed the USB device')
`,
  );
  await page.evaluate(() => {
    peerTest.session.abandon();
    peerTest.session.free();
  });
  await page.waitForFunction(() => peerTest.logs.some((v) => v.includes('completion unknown')));
  await native(
    setup +
      String.raw`
try:
 blocking.Session.open(devices[0],choices[0],limits)
except usb.UsbError as error:
 assert 'completion unknown' in str(error),str(error)
else:
 raise AssertionError('abandoned USB lease allowed another controller')
`,
  );
  await page.evaluate(() => peerTest.host.stop());
  assert.deepEqual(errors, []);
  console.log('PASS single-controller ownership and fencing after an abandoned peer session');
} finally {
  await browser.close();
}
