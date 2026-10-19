/** Exercise screenshot and hover playback against the actual catalog media. */
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { chromium, webkit } from 'playwright';

const base = process.env.FUJIMOD_WEB_URL;
if (!base) throw Error('Set FUJIMOD_WEB_URL.');
const output = new URL('../../build/features/', import.meta.url);
await mkdir(output, { recursive: true });

for (const engine of [chromium, webkit]) {
  const browser = await engine.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
    page.setDefaultTimeout(15000);
    const errors: string[] = [];
    const videos: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('request', (request) => {
      if (request.url().endsWith('.mp4')) videos.push(request.url());
    });
    console.log(`${engine.name()}: opening catalog`);
    await page.goto(new URL('/#features', base).href);
    await page.waitForFunction(
      () => performance.getEntriesByName('camera-catalog-ready').length > 0,
    );
    const previews = page.locator('.catalog-tile-preview');
    assert.ok((await previews.count()) > 0);
    assert.equal(videos.length, 0, 'Idle tiles must not download recordings');
    const tiles = page.locator('.catalog-tile').filter({ has: page.locator('video') });
    for (let index = 0; index < (await tiles.count()); index++) {
      const tile = tiles.nth(index);
      console.log(`${engine.name()}: preview ${index + 1}`);
      await tile.scrollIntoViewIfNeeded();
      await tile.locator('img').evaluate(async (image) => {
        await (image as HTMLImageElement).decode();
      });
      const video = tile.locator('video');
      assert.equal(await video.evaluate((video) => video.getAttribute('src')), null);
      assert.equal(await video.evaluate((video) => getComputedStyle(video).opacity), '0');
      if (index === 0)
        await page.screenshot({ path: new URL(`${engine.name()}-still.png`, output).pathname });
      // Hover the image so the separate choice popovers do not obscure the tile.
      await tile.locator('img').hover();
      await page.waitForFunction((index) => {
        const video = document.querySelectorAll<HTMLVideoElement>('.catalog-tile-video')[index];
        return !video.paused && video.currentTime > 0 && getComputedStyle(video).opacity === '1';
      }, index);
      if (index === 0)
        await page.screenshot({ path: new URL(`${engine.name()}-hover.png`, output).pathname });
      await page.mouse.move(0, 0);
      assert.equal(await video.evaluate((video) => (video as HTMLVideoElement).paused), true);
      assert.equal(await video.evaluate((video) => (video as HTMLVideoElement).currentTime), 0);
      assert.equal(await video.evaluate((video) => getComputedStyle(video).opacity), '0');
    }
    // Keyboard focus exercises a tile without a hover-card title.
    const recipe = tiles.filter({ hasText: 'Unlimited Recipes' });
    await recipe.focus();
    await page.waitForFunction(() => {
      const video = document.activeElement?.querySelector('video');
      return video instanceof HTMLVideoElement && !video.paused && video.currentTime > 0;
    });
    await recipe.evaluate((element) => (element as HTMLElement).blur());
    assert.equal(
      await recipe.locator('video').evaluate((video) => (video as HTMLVideoElement).paused),
      true,
    );
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator('#features').scrollIntoViewIfNeeded();
    assert.equal(
      await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
      true,
    );
    await page.screenshot({ path: new URL(`${engine.name()}-mobile.png`, output).pathname });
    assert.deepEqual(errors, []);
    console.log(
      `${engine.name()}: ${await tiles.count()} screenshot/hover previews and keyboard focus passed`,
    );
  } finally {
    await browser.close();
  }
}
