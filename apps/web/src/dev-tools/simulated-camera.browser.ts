/** Real stock execution check. The server supplies a qualified USB Wasm graph and DAT. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import type { Catalog } from '../catalog';
import type { CameraStore } from '../cameras/store';
import type { CameraModel } from './camera-model';
declare global {
  interface Window {
    __fujimodCameraModel?: CameraModel;
  }
}

const url = new URL('dev-tools/', process.env.FUJIMOD_WEB_URL || 'http://127.0.0.1:5204/');
const browser = await chromium.launch({ headless: true });
const directory = new URL('../../build/simulated-camera/', import.meta.url);
await mkdir(directory, { recursive: true });
try {
  const page = await browser.newPage({ viewport: { width: 1300, height: 900 } });
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });
  await page.goto(url.href);
  const savedId = await page.evaluate(async () => {
    const moduleUrl = new URL('/src/cameras/store.ts', location.href).href;
    const module = (await import(/* @vite-ignore */ moduleUrl)) as {
      CameraStore: typeof CameraStore;
    };
    const catalog = (await (await fetch('/runtime/catalog.json')).json()) as Catalog;
    const firmware = catalog.firmwares.find((entry) => entry.emulate?.usb)!;
    if (!firmware) throw Error('No qualified USB runtime in the catalog.');
    const bytes = await (await fetch('/local/firmware.dat')).arrayBuffer();
    return (await new module.CameraStore().create(new File([bytes], 'camera.dat'), firmware, false))
      .id;
  });
  await page.reload();
  await page.waitForFunction(() => 'connectSimulated' in (window.__fujimodCameraModel ?? {}));
  const tile = page.locator('.td-simulated');
  await tile.getByRole('button').click();
  const state = await page.waitForFunction(
    () => {
      const model = (window as Window & { __fujimodCameraModel?: CameraModel })
        .__fujimodCameraModel;
      const camera = model?.cameras().find((camera) => camera.transport === 'simulated');
      const error = document.querySelector('[role=alert]')?.textContent;
      if (error) throw Error(error);
      if (camera?.problem) throw Error(camera.problem);
      return camera?.identity?.product && !camera.session ? camera : undefined;
    },
    undefined,
    { timeout: 180_000 },
  );
  const camera = await state.jsonValue();
  assert.ok(camera?.identity);
  assert.equal(camera.id, `simulated-${savedId}`);
  assert.equal(camera.present, true);
  assert.equal(new URL(page.url()).pathname, url.pathname);
  const proof = await page.evaluate(async () => {
    const model = window.__fujimodCameraModel!;
    const camera = model.cameras().find((camera) => camera.transport === 'simulated')!;
    const identity = await model.withSession(
      camera.id,
      'Browser identity verification',
      (session) => session.deviceInfo('Product'),
    );
    const entries = model
      .log()
      .filter(
        (entry) =>
          entry.camera === camera.id && entry.message.startsWith('Device info · Product →'),
      );
    if (!entries.length) throw Error('No Rust-owner decoded identity exchange.');
    return { identity: identity.text, entries, camera: model.cameras()[0] };
  });
  assert.equal(proof.identity, camera.identity.product);
  assert.ok(
    proof.entries.every((entry) => entry.sizes && entry.sizes.sent > 0 && entry.sizes.received > 0),
  );
  await page.screenshot({ path: new URL('connected.png', directory).pathname, fullPage: true });
  await page.evaluate(async () => {
    const model = window.__fujimodCameraModel!;
    await model.remove(model.cameras().find((camera) => camera.transport === 'simulated')!.id);
    if (model.cameras().some((camera) => camera.transport === 'simulated'))
      throw Error('Simulation was not removed.');
  });
  assert.deepEqual(errors, []);
  await writeFile(new URL('identity.json', directory), JSON.stringify(proof, null, 2));
  console.log(
    `PASS saved camera ${savedId}: ${proof.identity}, Rust owner log, no navigation, removal`,
  );
} finally {
  await browser.close();
}
