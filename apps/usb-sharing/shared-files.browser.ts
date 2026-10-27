/** Verify the actual Worker response prevents SVG scripts in native browser contexts. */
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { runtime } from './test-runtime.ts';

const require = createRequire(new URL('../web/package.json', import.meta.url));
const { chromium, webkit } =
  require('playwright') as typeof import('../web/node_modules/playwright/index.js');
const mf = await runtime();
try {
  const bucket = await mf.getR2Bucket('SHARED_FILES');
  await bucket.put(
    'hostile.svg',
    '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" onload="document.documentElement.setAttribute(\'data-executed\',\'yes\')"><script>document.documentElement.setAttribute("data-executed","yes")</script><rect width="10" height="10" fill="red"/></svg>',
    {
      httpMetadata: { contentType: 'image/svg+xml' },
    },
  );
  for (const engine of [chromium, webkit]) {
    const browser = await engine.launch();
    try {
      const page = await browser.newPage();
      const base = await mf.ready;
      const response = await page.goto(new URL('/file/hostile.svg?direct=true', base).href);
      assert.equal(response?.status(), 200);
      assert.equal(await page.locator('svg').getAttribute('data-executed'), null);
      await page.goto(new URL('/file/hostile.svg', base).href);
      await page.locator('img').waitFor();
      assert.equal(
        await page
          .locator('img')
          .evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0),
        true,
      );
      console.log(
        `${engine.name()}: SVG displays as an image and cannot execute scripts when opened directly`,
      );
    } finally {
      await browser.close();
    }
  }
} finally {
  await mf.dispose();
}
