// Exercise the page with the real TAR produced by a passed camera walkthrough.
import { chromium, webkit } from 'playwright';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';
import assert from 'node:assert/strict';
const directory = process.argv[2],
  base = process.env.FUJIMOD_WEB_URL;
if (!directory || !base)
  throw Error('Set FUJIMOD_WEB_URL and pass the completed camera guide directory.');
const output = fileURLToPath(new URL('../../build/bug-report/', import.meta.url));
await mkdir(output, { recursive: true });
const guide = JSON.parse(await readFile(`${directory}/guide.json`, 'utf8'));
assert.equal(guide.schema, 2);
assert.equal(guide.passed, true);
assert.equal(guide.identity.walkthrough, 'bug-report/report/capture');
const url = new URL(guide.bug_report.issue_url);
const destination = new URL(base);
url.protocol = destination.protocol;
url.host = destination.host;
await writeFile(`${output}/preview-url.txt`, url.href);
const engine = process.env.FUJIMOD_BROWSER === 'webkit' ? webkit : chromium;
const browser = await engine.launch({ headless: true });
try {
  const context = await browser.newContext({
    acceptDownloads: true,
    viewport: { width: 390, height: 844 },
  });
  const sent = [],
    issues = [],
    errors = [];
  context.on('request', (request) => {
    if (!['GET', 'HEAD'].includes(request.method())) sent.push(request.url());
  });
  let issueArrives;
  const issueDelivered = new Promise((resolve) => {
    issueArrives = resolve;
  });
  await context.route('https://github.com/**', (route) => {
    issues.push(route.request().url());
    issueArrives();
    return route.fulfill({ status: 200, body: 'Intercepted issue preview; nothing submitted.' });
  });
  const page = await context.newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(url.href);
  await page.getByText('Connect your camera', { exact: true }).waitFor();
  await page.getByText('Nothing is uploaded automatically.', { exact: true }).waitFor();
  await page.screenshot({ path: `${output}/browser-phone.png`, fullPage: true });
  assert.equal(await page.locator('body').evaluate((e) => e.scrollWidth > innerWidth), false);
  await page.locator('input[type=file]').setInputFiles(`${directory}/report.tar`);
  await page.getByText('Report ready', { exact: true }).waitFor();
  await page.getByRole('textbox').fill('The menu stopped responding after a mod callback.');
  const button = page.getByRole('button', { name: 'Download and report', exact: true });
  await button.waitFor();
  await page.waitForFunction(() => !document.querySelector('button[type=submit]').disabled);
  const downloadPromise = page.waitForEvent('download');
  await button.click();
  const download = await downloadPromise;
  await download.saveAs(`${output}/${download.suggestedFilename()}`);
  assert.deepEqual(
    gunzipSync(await readFile(`${output}/${download.suggestedFilename()}`)),
    await readFile(`${directory}/report.tar`),
  );
  let timeout;
  try {
    await Promise.race([
      issueDelivered,
      new Promise((_, reject) => {
        timeout = setTimeout(() => reject(Error('Issue page did not open.')), 10000);
      }),
    ]);
  } finally {
    clearTimeout(timeout);
  }
  assert.equal(issues.length, 1);
  assert.ok(issues[0].includes('camera-bug.yml'));
  assert.ok(issues[0].includes('The+menu+stopped'));
  assert.deepEqual(sent, []);
  assert.deepEqual(errors, []);
  await page.screenshot({ path: `${output}/browser-ready-phone.png`, fullPage: true });
  await page.setViewportSize({ width: 1280, height: 960 });
  await page.screenshot({ path: `${output}/browser-ready-desktop.png`, fullPage: true });
  await writeFile(
    `${output}/browser-check.json`,
    JSON.stringify(
      {
        passed: true,
        url: url.href,
        archiveBytes: guide.bug_report.bytes,
        download: download.suggestedFilename(),
        outgoingWrites: sent,
        pageErrors: errors,
      },
      null,
      2,
    ),
  );
  console.log(
    'PASS',
    engine.name(),
    'real camera walkthrough TAR, responsive page, private local processing and exact downloaded bytes.',
  );
} finally {
  await browser.close();
}
