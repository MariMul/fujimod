/** Real firmware and renderer walkthrough against the configured development preview. */
import { chromium, webkit, type Page } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const base = process.env.FUJIMOD_WEB_URL;
const dat = process.env.FUJIMOD_TEST_DAT;
if (!base || !dat) throw Error('Set FUJIMOD_WEB_URL and FUJIMOD_TEST_DAT.');
const output = new URL('../../build/viewport-controls/', import.meta.url);
await mkdir(output, { recursive: true });

async function screenshot(page: Page, name: string) {
  await page.screenshot({ path: new URL(`${name}.png`, output).pathname, fullPage: true });
}
async function ready(page: Page) {
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
    () => !document.querySelector<HTMLButtonElement>('#firmware-mode button')?.disabled,
  );
}
async function menu(page: Page) {
  await page.locator('#viewer-settings-toggle').click();
  assert.equal(
    await page
      .locator('.viewport-settings')
      .evaluate((element) => (element as HTMLDetailsElement).open),
    true,
  );
}
async function state(page: Page) {
  return JSON.parse((await page.locator('#viewer').getAttribute('data-simulation')) ?? '{}') as {
    observerPosition: number[];
    mode: string;
    renderer: string;
  };
}
const browser = await chromium.launch({ headless: true });
let activePage: Page | undefined;
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  // Pin each navigation to its loaded source while other development work continues.
  await context.routeWebSocket('**/*', () => {});
  const page = await context.newPage(),
    errors: string[] = [];
  activePage = page;
  page.setDefaultTimeout(60000);
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (/\[[A-Z_]+\]/.test(message.text())) errors.push(message.text());
  });
  await page.goto(base, { waitUntil: 'domcontentloaded' });
  await page.locator('#viewer-tools[data-ready="true"]').waitFor();
  assert.equal(await page.getByRole('button', { name: 'New camera', exact: true }).count(), 0);
  assert.equal(await page.locator('#idle-cameras').count(), 0);
  await page.locator('#workspace').scrollIntoViewIfNeeded();
  await screenshot(page, 'first-use');
  await menu(page);
  await page.locator('#feedback-open').click();
  await page.locator('#feedback-description').fill('Viewport walkthrough');
  assert.equal(
    await page.locator('#feedback-dialog').evaluate((e) => (e as HTMLDialogElement).open),
    true,
  );
  await page.locator('#feedback-dialog button[value="cancel"]').click();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('#firmware-prompt').click();
  await (await chooser).setFiles(dat);
  await page.locator('#view-lcd').click();
  await ready(page);
  console.log('Created first camera from native file chooser and booted firmware');
  assert.equal(
    await page.getByRole('button', { name: 'New camera', exact: true }).isVisible(),
    true,
  );
  assert.equal(
    await page.locator('#lcd-source').evaluate((canvas: HTMLCanvasElement) =>
      canvas
        .getContext('2d')!
        .getImageData(0, 0, canvas.width, canvas.height)
        .data.some((value) => value > 0),
    ),
    true,
  );
  await page.locator('#view-3d').click();
  await page.waitForFunction(
    () =>
      JSON.parse(document.querySelector<HTMLElement>('#viewer')?.dataset.simulation ?? '{}').ready,
  );
  for (const width of [320, 390, 820, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.locator('#viewer-tools').scrollIntoViewIfNeeded();
    await menu(page);
    const bounds = await page
      .locator('#viewer-tools,.viewport-settings-panel')
      .evaluateAll((elements) =>
        elements.map((element) => {
          const rect = element.getBoundingClientRect();
          return { left: rect.left, right: rect.right, top: rect.top };
        }),
      );
    assert.ok(
      bounds.every((rect) => rect.left >= -1 && rect.right <= width + 1 && rect.top >= 0),
      JSON.stringify({ width, bounds }),
    );
    assert.equal(
      await page.evaluate(() => document.documentElement.scrollWidth > innerWidth),
      false,
      JSON.stringify(
        await page.evaluate(() => ({
          width: innerWidth,
          scroll: document.documentElement.scrollWidth,
          outside: [...document.querySelectorAll('*')]
            .map((element) => ({
              tag: element.tagName,
              id: element.id,
              class: element.getAttribute('class'),
              right: element.getBoundingClientRect().right,
            }))
            .filter((item) => item.right > innerWidth + 1)
            .slice(0, 25),
        })),
      ),
    );
    await screenshot(page, `layout-${width}`);
    await page.locator('#viewer-settings-toggle').press('Escape');
  }
  console.log('Responsive viewport geometry passed');
  await page.locator('#face-screen').click();
  const screen = await state(page);
  await page.locator('#face-lens').click();
  await page.waitForFunction(
    (before) =>
      document.querySelector<HTMLElement>('#viewer')?.dataset.simulation &&
      JSON.stringify(
        JSON.parse(document.querySelector<HTMLElement>('#viewer')!.dataset.simulation!)
          .observerPosition,
      ) !== JSON.stringify(before),
    screen.observerPosition,
  );
  await screenshot(page, 'inspect');
  await page.getByRole('button', { name: 'Walk', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>('#viewer')?.dataset.mode === 'walk',
  );
  await page.keyboard.press('Escape');
  await screenshot(page, 'walk');
  await page.getByRole('button', { name: 'Inspect', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelector<HTMLElement>('#viewer')?.dataset.mode === 'inspect',
  );
  await menu(page);
  await screenshot(page, 'settings');
  await page.locator('#viewer-settings-toggle').press('Escape');
  assert.equal(
    await page.locator('.viewport-settings').evaluate((e) => (e as HTMLDetailsElement).open),
    false,
  );
  await menu(page);
  await page.locator('.camera-library-heading h2').click();
  assert.equal(
    await page.locator('.viewport-settings').evaluate((e) => (e as HTMLDetailsElement).open),
    false,
  );
  for (const label of ['Unmodded', 'FujiMod']) {
    await page.locator('#view-lcd').click();
    await menu(page);
    await page.getByRole('button', { name: label, exact: true }).click();
    await ready(page);
    console.log(`${label} boot passed`);
    await menu(page);
    assert.equal(
      await page.getByRole('button', { name: label, exact: true }).getAttribute('aria-pressed'),
      'true',
    );
    await screenshot(page, label.toLowerCase());
    await page.locator('#viewer-settings-toggle').press('Escape');
  }
  await page.locator('#view-3d').click();
  // The shareable preview executes the same checked native boot from a fixed initial state.
  const preview = new URL('?scenario=x70-inspect#workspace', base).href;
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(preview, { waitUntil: 'domcontentloaded' });
  await ready(page);
  assert.equal(await page.locator('#view-3d').getAttribute('aria-pressed'), 'true');
  await page.waitForFunction(
    () =>
      JSON.parse(document.querySelector<HTMLElement>('#viewer')?.dataset.simulation ?? '{}').ready,
  );
  await screenshot(page, 'shared-preview');
  assert.deepEqual(errors, []);
  await writeFile(
    new URL('result.json', output),
    JSON.stringify(
      {
        preview,
        errors,
        controls: [
          '3D',
          'LCD',
          'Screen',
          'Lens',
          'Inspect',
          'Walk',
          'FujiMod',
          'Unmodded',
          'Feedback',
        ],
        widths: [320, 390, 820, 1440],
      },
      null,
      2,
    ),
  );
  console.log(
    'Passed: first upload, real native boots, views/orientations/navigation, both firmware modes, feedback, menu dismissal, responsive layouts and shared preview',
  );
} catch (error) {
  if (activePage) await screenshot(activePage, 'failure').catch(() => {});
  console.error(error);
  throw error;
} finally {
  await browser.close();
}

const safari = await webkit.launch();
try {
  const page = await safari.newPage({ viewport: { width: 390, height: 844 } });
  await page.goto(base, { waitUntil: 'domcontentloaded' });
  await page.locator('#viewer-tools[data-ready="true"]').waitFor();
  assert.equal(await page.getByRole('button', { name: 'New camera', exact: true }).count(), 0);
  await menu(page);
  await page.locator('#feedback-open').click();
  assert.equal(
    await page.locator('#feedback-dialog').evaluate((e) => (e as HTMLDialogElement).open),
    true,
  );
  await screenshot(page, 'webkit-feedback');
  console.log('Passed: WebKit first-use menu and feedback');
} finally {
  await safari.close();
}
