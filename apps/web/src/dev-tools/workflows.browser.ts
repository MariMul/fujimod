/** Execute service preparation and state capture through simulated stock USB. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import type { CameraModel } from './camera-model';
import type { CameraStore } from '../cameras/store';
import type { Catalog } from '../catalog';
import type { createUsbCameraModel } from './usb-camera-model';

declare global {
  interface Window {
    __workflowCamera?: CameraModel;
  }
}
const url = process.env.FUJIMOD_WEB_URL || 'http://127.0.0.1:5201/';
const firmwareId = process.env.FUJIMOD_TEST_FIRMWARE || 'x70-110';
const browser = await chromium.launch({ headless: true });
const output = new URL('../../build/workflows/', import.meta.url);
await mkdir(output, { recursive: true });
try {
  const page = await browser.newPage();
  page.setDefaultTimeout(180_000);
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') {
      errors.push(message.text());
      console.error(message.text());
    }
  });
  await page.addInitScript(() => {
    const NativeWorker = Worker;
    globalThis.Worker = class extends NativeWorker {
      constructor(...args: ConstructorParameters<typeof Worker>) {
        super(...args);
        this.addEventListener('message', (event: MessageEvent<{ type?: string }>) => {
          if (event.data?.type === 'error') console.error(JSON.stringify(event.data));
        });
      }
    };
  });
  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 90_000 });
  const savedId = await page.evaluate(async (firmwareId) => {
    const module = (await import(
      /* @vite-ignore */ new URL('/src/cameras/store.ts', location.href).href
    )) as { CameraStore: typeof CameraStore };
    const catalog = (await (await fetch('/runtime/catalog.json')).json()) as Catalog;
    const firmware = catalog.firmwares.find((f) => f.id === firmwareId);
    if (!firmware?.emulate?.usb) throw Error(`${firmwareId} has no supported simulated USB graph`);
    const response = await fetch(`/local/firmware/${firmwareId}.dat`);
    if (!response.ok) throw Error('Firmware mount unavailable');
    return (
      await new module.CameraStore().create(
        new File([await response.arrayBuffer()], 'camera.dat'),
        firmware,
        false,
      )
    ).id;
  }, firmwareId);
  console.log(`Boot ${firmwareId}`);
  await page.evaluate(async (id) => {
    const module = (await import(
      /* @vite-ignore */ new URL('/src/dev-tools/usb-camera-model.ts', location.href).href
    )) as { createUsbCameraModel: typeof createUsbCameraModel };
    const model = module.createUsbCameraModel({ discover: async () => [] });
    window.__workflowCamera = model;
    await model.connectSimulated!(id);
  }, savedId);
  await page.waitForFunction(
    () => {
      const camera = window.__workflowCamera?.cameras()[0];
      if (camera?.problem) throw Error(camera.problem);
      return camera?.identity?.product && !camera.session;
    },
    undefined,
    { timeout: 180_000 },
  );
  console.log('Prepare service session');
  const preparation = await page.evaluate(async (firmwareId) => {
    const model = window.__workflowCamera!;
    return model.withSession(model.cameras()[0].id, 'workflow verification', async (session) => {
      const original = await session.readConfiguration(0xa3, 1);
      const debug = await session.readConfiguration(0xf7, 1);
      const preparation = await session.prepareServiceSession(firmwareId);
      if (preparation.guardBefore !== original[0] || preparation.debugBefore !== debug[0])
        throw Error('Original flags differ');
      if (
        (await session.readConfiguration(0xa3, 1))[0] !== 1 ||
        (await session.readConfiguration(0xf7, 1))[0] !== 1
      )
        throw Error('Session flags were not retained');
      return preparation;
    });
  }, firmwareId);
  console.log('Dump catalog state');
  const regions = await page.evaluate(async (firmwareId) => {
    const model = window.__workflowCamera!;
    const regions: {
      name: string;
      address: number;
      size: number;
      constant: boolean;
      sha256: string;
    }[] = [];
    await model.withSession(model.cameras()[0].id, 'state verification', (session) =>
      session.dumpState(firmwareId, (piece) => {
        if (piece.data.length !== piece.size) throw Error('Region length differs');
        regions.push({
          name: piece.name,
          address: piece.address,
          size: piece.size,
          constant: piece.constant,
          sha256: piece.sha256,
        });
      }),
    );
    return regions;
  }, firmwareId);
  assert.ok(regions.some((r) => !r.constant && r.size > 0));
  assert.ok(regions.every((r) => /^[a-f0-9]{64}$/.test(r.sha256)));
  assert.deepEqual(errors, []);
  await writeFile(
    new URL(`${firmwareId}.json`, output),
    JSON.stringify({ preparation, regions }, null, 2),
  );
  await page.evaluate(() =>
    window.__workflowCamera!.remove(window.__workflowCamera!.cameras()[0].id),
  );
  console.log(
    `PASS ${firmwareId}: session flags retained; ${regions.length} state regions delivered`,
  );
} finally {
  await browser.close();
}
