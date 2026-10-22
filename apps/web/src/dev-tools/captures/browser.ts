/** Real stock execution: a simulated X70 runs its stock bulk query path for a state capture. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import type { Catalog } from '../../catalog';
import type { CameraStore } from '../../cameras/store';
import type { CameraModel } from '../camera-model';
declare global {
  interface Window {
    __fujimodCameraModel?: CameraModel;
  }
}

const firmwareId = process.env.FUJIMOD_TEST_FIRMWARE || 'x70-110';
const url = new URL(
  'dev-tools/?tool=state-captures',
  process.env.FUJIMOD_WEB_URL || 'http://127.0.0.1:5204/',
);
const output = new URL(`../../../build/state-captures/${firmwareId}/`, import.meta.url);
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1300, height: 900 },
  acceptDownloads: true,
});
const page = await context.newPage();
const errors: string[] = [];
page.on('pageerror', (error) => errors.push(error.message));
try {
  await page.goto(url.href, { waitUntil: 'domcontentloaded', timeout: 90_000 });
  await page.waitForFunction(() => window.__fujimodCameraModel, undefined, { timeout: 90_000 });
  const savedId = await page.evaluate(async (id) => {
    const module = (await import(
      /* @vite-ignore */ new URL('/src/cameras/store.ts', location.href).href
    )) as { CameraStore: typeof CameraStore };
    const catalog = (await (await fetch('/runtime/catalog.json')).json()) as Catalog;
    const firmware = catalog.firmwares.find((entry) => entry.id === id);
    if (!firmware) throw Error(`No runtime for ${id}`);
    const response = await fetch(`/local/firmware/${id}.dat`);
    if (!response.ok) throw Error(`Firmware mount failed: ${id} (${response.status})`);
    const file = new File([await response.arrayBuffer()], 'camera.dat');
    return (await new module.CameraStore().create(file, firmware, false)).id;
  }, firmwareId);
  await page.reload({ waitUntil: 'domcontentloaded', timeout: 90_000 });
  await page.waitForFunction(() => 'connectSimulated' in (window.__fujimodCameraModel ?? {}));
  await page.evaluate((id) => window.__fujimodCameraModel!.connectSimulated!(id), savedId);
  await page.waitForFunction(
    () => {
      const camera = window.__fujimodCameraModel?.cameras()[0];
      if (camera?.problem) throw Error(camera.problem);
      return camera?.identity?.internalVersion && !camera.session;
    },
    undefined,
    { timeout: 240_000 },
  );
  const row = page.locator('.cap-row', { hasText: 'Still shooting' });
  await row.getByRole('button', { name: 'Capture' }).click();
  await row.locator('.cap-count').waitFor({ timeout: 600_000 });
  await page.screenshot({ path: new URL('captured.png', output).pathname, fullPage: true });
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('button', { name: 'Download captures' }).click(),
  ]);
  const path = new URL('captures.tar', output).pathname;
  await download.saveAs(path);
  const tar = await readFile(path);
  const size = parseInt(tar.subarray(124, 136).toString().replace(/\0.*$/s, ''), 8);
  const manifest = JSON.parse(tar.subarray(512, 512 + size).toString()) as {
    firmware_id: string;
    states: { label: string; ranges: { size: number; pieces: { sha256: string }[] }[] }[];
  };
  assert.equal(manifest.firmware_id, firmwareId);
  assert.equal(manifest.states[0].label, 'Still shooting');
  const bytes = manifest.states[0].ranges.reduce((total, range) => total + range.size, 0);
  assert.ok(tar.length > bytes, 'archive holds the captured bytes');
  assert.deepEqual(errors, []);
  await writeFile(new URL('manifest.json', output), JSON.stringify(manifest, null, 2));
  console.log(`PASS ${firmwareId}: ${bytes} bytes captured through the stock bulk path`);
} catch (error) {
  await page.screenshot({ path: new URL('failure.png', output).pathname, fullPage: true });
  throw error;
} finally {
  await browser.close();
}
