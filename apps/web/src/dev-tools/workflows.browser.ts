/** Execute camera workflows through the saved-camera tile and simulated stock USB. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import type { CameraModel } from './camera-model';
import type { CameraStore } from '../cameras/store';
import type { Catalog } from '../catalog';
import type { WasmWorkerEvent } from '@fujimod/emulator-ui/engine/wasm-worker-protocol';

declare global {
  interface Window {
    __fujimodCameraModel?: CameraModel;
    __workflowWorker?: Worker;
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
  page.on('framenavigated', (frame) => {
    if (frame === page.mainFrame()) console.log('Page', frame.url());
  });
  const errors: string[] = [];
  page.on('pageerror', (error) => {
    errors.push(error.message);
    console.error(error.stack);
  });
  page.on('console', (message) => {
    if (message.text().startsWith('Flash backup: ')) console.log(message.text());
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
        if (String(args[0]).includes('/wasm-worker.ts')) window.__workflowWorker = this;
        this.addEventListener('message', (event: MessageEvent<{ type?: string }>) => {
          if (event.data?.type === 'error') console.error(JSON.stringify(event.data));
        });
      }
    };
  });
  await page.goto(new URL('dev-tools/', url).href, {
    waitUntil: 'domcontentloaded',
    timeout: 90_000,
  });
  await page.waitForFunction(() => Boolean(window.__fujimodCameraModel));
  await page.waitForLoadState('networkidle');
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
  await page.reload({ waitUntil: 'domcontentloaded', timeout: 90_000 });
  await page.waitForFunction(() => Boolean(window.__fujimodCameraModel));
  console.log(`Click simulated ${firmwareId} tile`);
  await page.locator('.td-saved-camera').last().click();
  await page.waitForFunction(
    () => {
      const camera = window.__fujimodCameraModel?.cameras()[0];
      if (camera?.problem) throw Error(camera.problem);
      return camera?.identity?.product && !camera.session;
    },
    undefined,
    { timeout: 180_000 },
  );
  console.log('Prepare service session');
  const preparation = await page.evaluate(async (firmwareId) => {
    const model = window.__fujimodCameraModel!;
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
  console.log('Check flash backup');
  const backup = await page.evaluate(
    async ({ firmwareId, savedId }) => {
      const model = window.__fujimodCameraModel!;
      const catalog = (await (await fetch('/runtime/catalog.json')).json()) as Catalog;
      const available = Boolean(catalog.firmwares.find((f) => f.id === firmwareId)?.flash_reader);
      const pieces: { offset: number; data: Uint8Array }[] = [];
      let delivered = 0;
      const digest = async (bytes: ArrayBuffer | Uint8Array<ArrayBuffer>) =>
        Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), (byte) =>
          byte.toString(16).padStart(2, '0'),
        ).join('');
      let serial = -1;
      const durable = async () => {
        const worker = window.__workflowWorker;
        if (!worker) throw Error('Simulation worker was not observed');
        const request = serial--;
        await new Promise<void>((resolve, reject) => {
          const reply = (event: MessageEvent<WasmWorkerEvent>) => {
            if (
              (event.data.type === 'reply' || event.data.type === 'error') &&
              event.data.serial === request
            ) {
              worker.removeEventListener('message', reply);
              if (event.data.type === 'error') reject(Error(event.data.error));
              else resolve();
            }
          };
          worker.addEventListener('message', reply);
          worker.postMessage({ type: 'rpc', serial: request, request: { action: 'save' } });
        });
        const module = (await import(
          /* @vite-ignore */ new URL('/src/cameras/store.ts', location.href).href
        )) as { CameraStore: typeof CameraStore };
        const save = await new module.CameraStore().saved(savedId);
        if (!save) throw Error('Durable state was not saved');
        return digest(save.durable);
      };
      const durableBefore = await durable();
      try {
        const result = await model.withSession(
          model.cameras()[0].id,
          'flash verification',
          (session) =>
            session.backupFlash(firmwareId, (piece) => {
              pieces.push({ offset: piece.offset, data: piece.data.slice() });
              delivered += piece.data.length;
              if (delivered % (16 * 1024 * 1024) === 0)
                console.log(`Flash backup: ${delivered / 1048576} MiB delivered`);
            }),
        );
        let end = 0;
        const image = new Uint8Array(result.size);
        for (const piece of pieces.sort((a, b) => a.offset - b.offset)) {
          if (piece.offset !== end) throw Error('Flash chunks do not cover a contiguous range');
          image.set(piece.data, piece.offset);
          end += piece.data.length;
        }
        if (end !== result.size) throw Error('Flash chunks do not cover the whole device');
        const independentSha256 = await digest(image);
        if (independentSha256 !== result.sha256) throw Error('Independent flash digest differs');
        await model.withSession(model.cameras()[0].id, 'cleanup verification', async (session) => {
          const prepared = await session.prepareServiceSession(firmwareId);
          if (prepared.guardBefore !== 1 || prepared.debugBefore !== 1)
            throw Error('Service flags changed during flash backup');
        });
        const durableAfter = await durable();
        if (durableAfter !== durableBefore)
          throw Error('Persistent storage changed during flash backup');
        return {
          status: 'complete',
          independentSha256,
          durableBefore,
          durableAfter,
          chunks: pieces.length,
          ...result,
        };
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        if (available || pieces.length || !reason.includes('no built flash reader')) throw error;
        return { status: 'unavailable', reason };
      }
    },
    { firmwareId, savedId },
  );
  console.log(`Flash backup: ${backup.status}`);
  if (process.env.FUJIMOD_TEST_STATE === '0') {
    await writeFile(
      new URL(`${firmwareId}-service.json`, output),
      JSON.stringify({ preparation, backup }, null, 2),
    );
    assert.deepEqual(errors, []);
    console.log(`PASS ${firmwareId}: service prepared; flash backup ${backup.status}`);
    await page.evaluate(() =>
      window.__fujimodCameraModel!.remove(window.__fujimodCameraModel!.cameras()[0].id),
    );
  } else {
    console.log('Dump catalog state');
    const regions = await page.evaluate(async (firmwareId) => {
      const model = window.__fujimodCameraModel!;
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
      JSON.stringify({ preparation, backup, regions }, null, 2),
    );
    await page.evaluate(() =>
      window.__fujimodCameraModel!.remove(window.__fujimodCameraModel!.cameras()[0].id),
    );
    console.log(
      `PASS ${firmwareId}: session flags retained; ${regions.length} state regions delivered`,
    );
  }
} finally {
  await browser.close();
}
