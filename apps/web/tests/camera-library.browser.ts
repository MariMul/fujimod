/** Real browser/firmware lifecycle. Supply a DAT and the running development URL. */
import type { Locator } from 'playwright';
declare global {
  interface Window {
    __cameraState: {
      image_frames: number;
      virtual_ticks: number;
      capture?: { completed: number };
      power: { on: boolean };
    };
    __cameraSaved: { savedAt?: number; error?: string };
    __cameraReady?: unknown;
    __cameraCache: { mode: string };
  }
}
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const url = process.env.FUJIMOD_WEB_URL,
  dat = process.env.FUJIMOD_TEST_DAT;
if (!url || !dat) throw Error('Set FUJIMOD_WEB_URL and FUJIMOD_TEST_DAT.');
const output = new URL('../build/camera-library/', import.meta.url);
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1080 } });
await context.addInitScript(() => {
  const Original = window.Worker;
  window.Worker = class extends Original {
    constructor(...args: ConstructorParameters<typeof Worker>) {
      super(...args);
      if (!String(args[0]).includes('wasm-worker')) return;
      this.addEventListener('message', ({ data }) => {
        if (data.state) window.__cameraState = { ...window.__cameraState, ...data.state };
        if (data.type === 'camera_saved') window.__cameraSaved = data;
        if (data.type === 'ready') window.__cameraReady = data;
        if (data.type === 'cache') window.__cameraCache = data;
      });
    }
  };
});
const page = await context.newPage(),
  errors: string[] = [];
page.setDefaultTimeout(60000);
page.on('pageerror', (error) => errors.push(error.message));
page.on('console', (message) => {
  if (/\[[A-Z_]+\]/.test(message.text())) errors.push(message.text());
});
const monitor = setInterval(() => {
  void page
    .evaluate(() => ({
      status: document.querySelector('#status')?.textContent,
      renderer: document.querySelector('#viewer')?.getAttribute('data-renderer-stage'),
      save: document.querySelector('.camera-storage-status')?.textContent,
    }))
    .then((state) => console.log('Lifecycle progress:', state))
    .catch(() => {});
}, 30000);
async function ready() {
  await page.waitForFunction(
    () =>
      document.querySelector('#live-label')?.textContent === 'LIVE' ||
      document.querySelector('#status')?.classList.contains('error'),
    undefined,
    { timeout: 240000 },
  );
  assert.equal(
    await page.locator('#live-label').innerText(),
    'LIVE',
    await page.locator('#status').innerText(),
  );
  await page.waitForFunction(
    () => window.__cameraSaved?.savedAt || window.__cameraSaved?.error,
    undefined,
    { timeout: 90000 },
  );
  assert.ok(
    !(await page.evaluate(() => window.__cameraSaved.error)),
    await page.locator('.camera-storage-status').innerText(),
  );
}
async function records() {
  return page.evaluate(async () => {
    const { CameraStore } = (await import(
      String('/src/cameras/store.ts')
    )) as typeof import('../src/cameras/store.ts');
    return new CameraStore().list();
  });
}
async function screenshot(name: string) {
  await page.screenshot({ path: new URL(name + '.png', output).pathname, fullPage: true });
}
async function photoHash(button: Locator) {
  const pending = page.waitForEvent('download');
  await button.click();
  const download = await pending;
  return createHash('sha256')
    .update(await readFile(await download.path()))
    .digest('hex');
}
async function setDial(id: string, value: string) {
  const dial = page.locator(`.dial[data-id="${id}"]`);
  const values = await dial
    .locator('.vals span')
    .evaluateAll((items) => items.map((item) => (item as HTMLElement).dataset.value));
  const target = values.indexOf(value);
  assert.ok(target >= 0);
  const current = Number(await dial.getAttribute('aria-valuenow'));
  await dial.focus();
  for (let step = 0; step < Math.abs(target - current); step++)
    await dial.press(target > current ? 'ArrowRight' : 'ArrowLeft');
  assert.equal(await dial.getAttribute('aria-valuetext'), value);
}
try {
  await page.goto(url);
  console.log('Opened camera library');
  await page.locator('#viewer-tools[data-ready="true"]').waitFor();
  assert.equal(await page.getByRole('button', { name: 'New camera', exact: true }).count(), 0);
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>('#viewer')?.dataset.renderer === 'lcd',
    undefined,
    { timeout: 60000 },
  );
  console.log('LCD ready');
  assert.equal(await page.locator('.saved-camera').count(), 0);
  assert.equal(await page.locator('#live-label').innerText(), '');
  const storageResult = await page.evaluate(async () => {
    const { CameraStore } = (await import(
      String('/src/cameras/store.ts')
    )) as typeof import('../src/cameras/store.ts');
    const store = new CameraStore(),
      firmware = { id: 'storage-check', model: 'Storage check', version: '1', sha256: 'test' };
    const file = new File([new Uint8Array([1, 2, 3])], 'test.dat');
    const a = await store.create(file, firmware, false),
      b = await store.create(file, firmware, false);
    const state = (n: number) => ({
      durable: new Uint8Array([n]).buffer,
      runtimeKey: 'test',
      poweredOn: false,
      positions: {},
      modEnabled: false,
    });
    await store.save(a.id, 0, state(7));
    await store.save(b.id, 0, state(9));
    let conflict = false;
    try {
      await store.save(a.id, 0, state(99));
    } catch {
      conflict = true;
    }
    // Fail after queuing the media write: the whole transaction must roll back.
    const put = Object.getOwnPropertyDescriptor(IDBObjectStore.prototype, 'put');
    if (!put) throw Error('The browser has no IndexedDB write method.');
    let quotaFailure = false;
    IDBObjectStore.prototype.put = function (...args) {
      if (this.name === 'cameras') throw new DOMException('Storage full', 'QuotaExceededError');
      return Reflect.apply(put.value, this, args);
    };
    try {
      await store.save(a.id, 1, state(99));
    } catch (error) {
      quotaFailure = error instanceof Error && error.name === 'QuotaExceededError';
    } finally {
      Object.defineProperty(IDBObjectStore.prototype, 'put', put);
    }
    return {
      different: a.id !== b.id,
      firmware: [...new Uint8Array(await (await store.firmware(a.id)).arrayBuffer())],
      a: [...new Uint8Array((await store.saved(a.id))!.durable)],
      b: [...new Uint8Array((await store.saved(b.id))!.durable)],
      conflict,
      quotaFailure,
      revision: (await store.list()).find((record) => record.id === a.id)!.revision,
    };
  });
  assert.deepEqual(storageResult, {
    different: true,
    firmware: [1, 2, 3],
    a: [7],
    b: [9],
    conflict: true,
    quotaFailure: true,
    revision: 1,
  });
  await page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const request = indexedDB.deleteDatabase('fujimod-cameras');
        request.onsuccess = resolve;
        request.onerror = reject;
      }),
  );
  await page.reload();
  await page.locator('#viewer-tools[data-ready="true"]').waitFor();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('#firmware-prompt').click();
  await (await chooser).setFiles(dat);
  console.log('Uploaded firmware');
  await ready();
  const first = (await records())[0];
  assert.ok(first.id && first.savedAt);
  assert.equal(await page.locator('.personal-note').count(), 0);
  await page.locator('#workspace').scrollIntoViewIfNeeded();
  await screenshot('running');
  console.log(
    'Initial snapshot bytes:',
    await page.evaluate(async (id) => {
      const { CameraStore } = (await import(
        String('/src/cameras/store.ts')
      )) as typeof import('../src/cameras/store.ts');
      return (await new CameraStore().saved(id))?.snapshot?.byteLength ?? 0;
    }, first.id),
  );
  console.log('Created camera, retained DAT, booted actual firmware and saved durable state');
  await page.waitForFunction(
    async (revision) => {
      const { CameraStore } = (await import(
        String('/src/cameras/store.ts')
      )) as typeof import('../src/cameras/store.ts');
      const store = new CameraStore(),
        record = (await store.list())[0];
      return record?.revision > revision && !!(await store.saved(record.id))?.snapshot;
    },
    first.revision,
    { timeout: 180000 },
  );
  for (let refresh = 0; refresh < 3; refresh++) {
    await page.reload();
    await page.locator('.saved-camera').waitFor();
    assert.equal(await page.locator('#live-label').innerText(), '');
    assert.equal(await page.locator('.saved-camera').count(), 1);
    assert.equal((await records())[0].id, first.id);
    assert.equal(await page.evaluate(() => window.__cameraReady), undefined);
  }
  await page.locator('.saved-camera').click();
  await ready();
  assert.equal(
    await page.evaluate(() => window.__cameraCache.mode),
    'resume',
    JSON.stringify(await page.evaluate(() => window.__cameraCache)),
  );
  console.log('Resumed a live snapshot of the owned camera');
  await page.locator('[data-id="focus_mode"] [data-value="M"]').click();
  await setDial('shutter_speed', '1/125');
  const settle = await page.evaluate(() => window.__cameraState.image_frames + 1);
  await page.waitForFunction((frame) => window.__cameraState.image_frames >= frame, settle, {
    timeout: 90000,
  });
  const captures = await page.evaluate(() => window.__cameraState.capture?.completed ?? 0);
  await page.locator('.slide[data-id="shutter"]').click();
  await screenshot('capture-start');
  console.log('Pressed the native shutter');
  await page.waitForFunction(
    (count) => (window.__cameraState.capture?.completed ?? 0) > count,
    captures,
    {
      timeout: 360000,
    },
  );
  const photo = page.locator('#card-files button[aria-label$=".JPG"]').first();
  await photo.waitFor();
  const photoLabel = await photo.getAttribute('aria-label');
  assert.ok(photoLabel);
  const capturedHash = await photoHash(photo);
  await screenshot('captured');
  console.log('Captured and downloaded a photo through the native camera controls');
  const cardFiles = await page.locator('#card-files').innerText();
  await page.locator('[data-id="power"] [data-value="off"]').click();
  await page.waitForFunction(() => window.__cameraState?.power?.on === false);
  await page.waitForFunction(
    async () => {
      const { CameraStore } = (await import(
        String('/src/cameras/store.ts')
      )) as typeof import('../src/cameras/store.ts');
      return (await new CameraStore().list())[0]?.poweredOn === false;
    },
    undefined,
    { timeout: 90000 },
  );
  const before = await page.evaluate(async (id) => {
    const { CameraStore } = (await import(
      String('/src/cameras/store.ts')
    )) as typeof import('../src/cameras/store.ts');
    const save = await new CameraStore().saved(id);
    if (!save) throw Error('Camera state was not saved');
    return { bytes: save.durable.byteLength, snapshot: !!save.snapshot, positions: save.positions };
  }, first.id);
  assert.ok(before.bytes > 0);
  assert.equal(before.snapshot, false);
  assert.equal(before.positions.power, 'off');
  await screenshot('off');
  await page.reload();
  await page.locator('.saved-camera').waitFor();
  assert.equal(await page.locator('#live-label').innerText(), '');
  assert.equal(await page.locator('.saved-camera').count(), 1);
  assert.equal(await page.locator('.personal-note').count(), 0);
  await screenshot('library');
  await page.locator('.saved-camera').click();
  await ready();
  assert.equal(await page.evaluate(() => window.__cameraState.power.on), false);
  assert.equal(await page.locator('#card-files').innerText(), cardFiles);
  assert.equal(
    await photoHash(page.getByRole('button', { name: photoLabel, exact: true })),
    capturedHash,
  );
  assert.equal(
    await page.locator('[data-id="power"] [data-value="off"]').getAttribute('aria-pressed'),
    'true',
  );
  await page.locator('[data-id="power"] [data-value="on"]').click();
  await page.waitForFunction(() => window.__cameraState?.power?.on === true, undefined, {
    timeout: 240000,
  });
  await page.getByRole('button', { name: 'Cameras', exact: true }).click();
  await page.locator('.saved-camera').waitFor();
  // Drop the user's persisted DAT through native DataTransfer, without new host inputs.
  await page.locator('body').evaluate(async (node, id) => {
    const { CameraStore } = (await import(
      String('/src/cameras/store.ts')
    )) as typeof import('../src/cameras/store.ts');
    const file = await new CameraStore().firmware(id),
      transfer = new DataTransfer();
    transfer.items.add(new File([file], 'FPUPDATE.DAT'));
    node.dispatchEvent(
      new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }),
    );
  }, first.id);
  await ready();
  const all = await records();
  assert.equal(all.length, 2);
  assert.notEqual(all[0].id, all[1].id);
  assert.equal(await page.locator('#card-files button[aria-label$=".JPG"]').count(), 0);
  await page.getByRole('button', { name: 'Cameras', exact: true }).click();
  await page.waitForFunction(() => document.querySelectorAll('.saved-camera').length === 2);
  await screenshot('two-cameras');
  assert.deepEqual(errors, []);
  console.log(
    'Passed: periodic save, photo survives OFF and reload byte-for-byte, restored OFF, power ON, independent camera from drop',
  );
} catch (error) {
  console.log(await page.locator('#status').innerText());
  console.log(await page.locator('#engine-status').textContent());
  console.log('Browser errors:', errors);
  console.log(
    'Last execution state:',
    await page.evaluate(() => ({
      tick: window.__cameraState?.virtual_ticks,
      frames: window.__cameraState?.image_frames,
      capture: window.__cameraState?.capture,
      power: window.__cameraState?.power,
    })),
  );
  await screenshot('failure');
  throw error;
} finally {
  clearInterval(monitor);
  await browser.close();
}
