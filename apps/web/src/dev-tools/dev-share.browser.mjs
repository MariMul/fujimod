// Real firmware simulator and installed relay; never opens physical USB.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { setTimeout } from 'node:timers/promises';
import { parseShare } from '../../../../crates/usb-proxy/web/peer.js';

if (!process.env.FUJIMOD_WEB_URL)
  throw Error('Set FUJIMOD_WEB_URL to the named development HTTPS URL');
const url = new URL('dev-tools/', process.env.FUJIMOD_WEB_URL);
url.searchParams.set('target', 'simulator');
url.searchParams.set('firmware', 'x70-110');
url.searchParams.set('connect', '1');
const run = crypto.randomUUID();
url.searchParams.set('dev-share-test', run);
const registry = new URL('../../../../target/usb-relay-service/dev-shares/', import.meta.url);
const output = new URL('../../build/dev-share-browser/', import.meta.url);
await mkdir(output, { recursive: true });
const otherShares = (await readdir(registry)).length;
const files = async () => {
  const entries = await Promise.all(
    (await readdir(registry))
      .filter((name) => name.endsWith('.json'))
      .map(async (name) => {
        try {
          const entry = JSON.parse(await readFile(new URL(name, registry), 'utf8'));
          return new URL(entry.url).searchParams.get('dev-share-test') === run ? name : undefined;
        } catch (error) {
          if (error.code === 'ENOENT') return;
          throw error;
        }
      }),
  );
  return entries.filter(Boolean);
};
const observedCounts = [];
async function count(expected) {
  const deadline = Date.now() + (expected ? 240000 : 15000);
  while (Date.now() < deadline) {
    const entries = await files();
    if (entries.length === expected) {
      observedCounts.push({
        expected,
        own: entries.length,
        total: (await readdir(registry)).filter((name) => name.endsWith('.json')).length,
      });
      return entries;
    }
    await setTimeout(100);
  }
  assert.equal((await files()).length, expected, 'Development registry entry count');
}

const browser = await chromium.launch({ headless: true });
const errors = [];
try {
  const context = await browser.newContext({ viewport: { width: 1100, height: 800 } });
  await context.grantPermissions(['local-network-access'], { origin: url.origin });
  await context.addInitScript(() => {
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
  const owner = await context.newPage();
  owner.on('pageerror', (error) => {
    errors.push(String(error));
    console.error(String(error));
  });
  let acknowledgements = 0;
  owner.on('websocket', (socket) => {
    if (new URL(socket.url()).pathname === '/usb-proxy/dev-share')
      socket.on('framereceived', ({ payload }) => {
        if (JSON.parse(String(payload)).type === 'registered') acknowledgements++;
      });
  });
  await owner.goto(url.href);
  await owner
    .getByText('Development share registered', { exact: true })
    .first()
    .waitFor({ timeout: 240000 });
  assert.equal((await owner.locator('.dt-status').textContent()).trim(), 'Sharing');
  assert.equal(await owner.getByLabel('Console command', { exact: true }).isDisabled(), true);
  const [file] = await count(1);
  assert.equal((await stat(new URL(file, registry))).mode & 0o777, 0o600);
  const first = JSON.parse(await readFile(new URL(file, registry), 'utf8'));
  const ticket = parseShare(first.url);
  console.log('Simulator auto-shared; one private registry entry');
  assert.equal(new URL(first.url).origin, url.origin);
  assert.equal(typeof first.camera.model, 'string');
  assert.ok(Object.hasOwn(first.camera, 'serial'));
  await owner.screenshot({ path: new URL('shared.png', output).pathname, fullPage: true });

  await owner.getByRole('button', { name: 'Share', exact: true }).click();
  await owner.getByRole('button', { name: 'Stop sharing', exact: true }).click();
  await count(0);
  console.log('Unshare withdrew the entry');
  await owner.screenshot({ path: new URL('unshared.png', output).pathname, fullPage: true });
  await owner.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await owner.getByRole('button', { name: 'Connect simulator', exact: true }).click();
  await count(1);
  const [secondFile] = await files();
  const second = parseShare(JSON.parse(await readFile(new URL(secondFile, registry), 'utf8')).url);
  assert.notEqual(ticket.room, second.room);
  assert.notEqual(ticket.key, second.key);
  await owner.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await count(0);
  await owner.getByRole('button', { name: 'Connect simulator', exact: true }).click();
  await count(1);
  await owner.close();
  await count(0);
  assert.equal(acknowledgements, 3);
  assert.deepEqual(errors, []);
  await writeFile(
    new URL('result.json', output),
    JSON.stringify(
      {
        passed: true,
        url: url.href,
        acknowledgements,
        otherShares,
        registryCounts: observedCounts,
        privateFiles: true,
        freshTickets: true,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS: real simulator auto-share, unshare, disconnect, reconnect and tab close; registry 1 → 0 → 1 → 0 → 1 → 0',
  );
} catch (error) {
  for (const [index, page] of browser
    .contexts()
    .flatMap((context) => context.pages())
    .entries()) {
    await writeFile(
      new URL(`failure-${index}.txt`, output),
      await page.locator('body').innerText(),
    ).catch(() => {});
    await page
      .screenshot({ path: new URL(`failure-${index}.png`, output).pathname })
      .catch(() => {});
  }
  throw error;
} finally {
  await browser.close();
  await count(0);
}
