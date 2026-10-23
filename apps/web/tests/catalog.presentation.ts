/** Check the camera progress table against the catalog's generated progress documents. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { glob, mkdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { byModel, phases, started, type FirmwareProgress } from '../src/ui/camera-progress.ts';

const url = process.env.FUJIMOD_WEB_URL;
const output = process.env.FUJIMOD_TEST_OUTPUT;
if (!url || !output) throw Error('Set the web URL and screenshot output directory.');
const catalog = fileURLToPath(new URL('../../../catalog/firmware/', import.meta.url));
const documents: FirmwareProgress[] = [];
for await (const path of glob('*/*/progress.generated.json', { cwd: catalog }))
  documents.push(JSON.parse(await readFile(catalog + path, 'utf8')));
const expected = Object.values(byModel(documents)).filter(started);
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1500, height: 1100 } });
  await page.context().routeWebSocket('**/*', () => {});
  await page.goto(new URL('#camera-table', url).href);
  const rows = page.locator('#camera-table .progress-row');
  assert.equal(await rows.count(), expected.length);
  for (const progress of expected) {
    const row = rows.filter({ has: page.getByText(progress.model, { exact: true }) });
    assert.equal(await row.count(), 1, progress.model);
    const list = phases(progress);
    const dot = row.locator('.phase.research');
    assert.equal(await dot.count(), 1, progress.model);
    assert.equal(
      await dot.evaluate((element) => element.classList.contains('partial')),
      list[0]?.fraction === 1 / 2,
      `${progress.model} research dot`,
    );
    for (const [index, phase] of list.entries()) {
      const segment = row.locator('.phase').nth(index);
      await segment.hover();
      const tip = segment.locator('.tip');
      await tip.waitFor({ state: 'visible' });
      assert.equal(await tip.locator('.tip-label').innerText(), phase.label, progress.model);
      assert.equal(
        await tip.locator('li:not(.todo)').count(),
        phase.steps.filter((step) => step.done).length,
        `${progress.model} ${phase.label}`,
      );
    }
  }
  await page.locator('#camera-table').screenshot({ path: `${output}/camera-progress.png` });
  console.log(`${expected.length} started cameras: progress rows and popovers verified at ${url}`);
} finally {
  await browser.close();
}
