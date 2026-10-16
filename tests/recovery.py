"""Headed browser recovery/cache gate. Requires Playwright and a local runtime.

Outputs go to a new caller-selected directory. No hardware access. The served
site may opt into local DAT loading, or --dat supplies an explicit input file.
"""
import argparse
import asyncio
import json
from pathlib import Path
import time

from playwright.async_api import async_playwright


async def check(playwright, args, name):
    out = args.output / name
    out.mkdir()
    browser = await getattr(playwright, name).launch(headless=False)
    options = playwright.devices['iPhone 13'] if name == 'webkit' else dict(viewport=dict(width=1440, height=1000))
    context = await browser.new_context(**options)
    await context.add_init_script('''(() => {
      const WorkerClass = window.Worker;
      window.__faults = []; window.__cache = undefined;
      window.Worker = class extends WorkerClass {
        constructor(...args) {
          super(...args);
          if (!String(args[0]).includes('pyodide-worker')) return;
          window.__engineWorker = this;
          this.addEventListener('message', ({data}) => {
            if (data.type === 'cache') window.__cache = data;
            if (data.type === 'ready') window.__ready = data;
            if (data.state) window.__state = data.state;
            if (data.type === 'error') window.__faults.push(data);
          });
        }
      };
    })();''')
    page = await context.new_page()
    page_errors = []
    page.on('pageerror', lambda error: page_errors.append(str(error)))
    results = []
    try:
        for phase in ('cold', 'warm'):
            started = time.perf_counter()
            await page.goto(args.url)
            if args.dat:
                await page.locator('#firmware-file').set_input_files(str(args.dat))
            await page.wait_for_function("document.querySelector('#live-label').textContent==='LIVE' || document.querySelector('#status').classList.contains('error')", timeout=180000)
            elapsed = time.perf_counter() - started
            assert await page.locator('#live-label').inner_text() == 'LIVE', await page.locator('#status').inner_text()
            await page.evaluate("window.__cacheTimer = setInterval(() => window.__engineWorker.postMessage({type:'recovery_status'}), 1000)")
            await page.wait_for_function('window.__cache?.saved || window.__cache?.cacheFailure', timeout=90000)
            cache = await page.evaluate('window.__cache')
            assert cache['saved'], cache
            assert cache['backend'] in ('opfs', 'indexeddb'), cache
            assert cache['mode'] == ('resume' if phase == 'warm' else 'cold'), cache
            if phase == 'warm':
                assert elapsed < args.max_warm_seconds, f'Warm reload took {elapsed:.3f}s (limit {args.max_warm_seconds:g}s)'
            results.append(dict(phase=phase, seconds=elapsed, cache=cache))
            await page.screenshot(path=str(out / f'{phase}.png'))
            await page.evaluate('clearInterval(window.__cacheTimer)')
        await page.evaluate("window.__cacheTimer = setInterval(() => window.__engineWorker.postMessage({type:'recovery_status'}), 1000)")
        await page.wait_for_function('''(() => {
          const points = window.__cache?.history?.snapshots || [];
          return points.length >= 6 && points.at(-1) - points[0] >= 120000000;
        })()''', timeout=180000)
        before = await page.evaluate('({state:window.__state, cache:window.__cache, performance:window.cameraPerformance})')
        # A real transport exception must follow the same pause/error path as
        # errors raised by device callbacks. Kind-specific normalization has
        # separate unit coverage for UnsupportedDevice, guest faults and traps.
        await page.evaluate("window.__engineWorker.postMessage({type:'request',id:-100,action:'advance',options:{limit:200001,pause_at_tick:0}})")
        await page.wait_for_function("document.querySelector('#live-label').textContent==='STOPPED'")
        pixels = await page.locator('#lcd-source').evaluate('c => c.toDataURL()')
        stopped_ticks = await page.evaluate('window.__state.virtual_ticks')
        await page.wait_for_timeout(1100)
        assert await page.locator('#lcd-source').evaluate('c => c.toDataURL()') == pixels
        assert await page.evaluate('window.__state.virtual_ticks') == stopped_ticks
        assert await page.locator('#fault-details').evaluate('e => !e.open')
        assert '{' not in await page.locator('#status').inner_text()
        await page.screenshot(path=str(out / 'fault.png'))
        await page.locator('#undo').click()
        await page.wait_for_function("document.querySelector('#live-label').textContent==='LIVE'")
        restored_ticks = await page.evaluate('window.__state.virtual_ticks')
        assert restored_ticks <= stopped_ticks - 120000000, (stopped_ticks, restored_ticks)
        await page.wait_for_function('(tick) => window.__state.virtual_ticks > tick', arg=restored_ticks)
        await page.screenshot(path=str(out / 'undo.png'))
        assert page_errors == [], page_errors
        result = dict(loads=results, before=before, stopped_ticks=stopped_ticks, restored_ticks=restored_ticks,
                      faults=await page.evaluate('window.__faults'), page_errors=page_errors)
        (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(dict(browser=name, loads=[r['seconds'] for r in results], stopped_ticks=stopped_ticks, restored_ticks=restored_ticks)), flush=True)
    finally:
        await browser.close()


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', default='http://127.0.0.1:5190/')
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--dat', type=Path)
    parser.add_argument('--max-warm-seconds', type=float, default=5,
                        help='Reload gate in seconds; the required target is 5')
    parser.add_argument('--engines', nargs='+', default=['chromium', 'webkit'])
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as playwright:
        for name in args.engines:
            await check(playwright, args, name)


if __name__ == '__main__':
    asyncio.run(main())
