"""Headed browser recovery/cache gate. Requires Playwright and a local runtime.

Outputs go to a new caller-selected directory. No hardware access. The served
site may opt into local DAT loading, or --dat supplies an explicit input file.
"""
import argparse
import asyncio
import json
from pathlib import Path
import time
import zipfile

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
            if phase == 'cold': await page.goto(args.url)
            else: await page.reload()
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
        assert await page.locator('#fault-raw').is_visible()
        assert await page.locator('#engine-status details').count()==0
        assert '{' not in await page.locator('#status').inner_text()
        await page.screenshot(path=str(out / 'fault.png'))
        async with page.expect_download(timeout=60000) as pending:
            await page.locator('#error-report').click()
        download=await pending.value
        report_path=out/'error-report.zip'
        await download.save_as(report_path)
        with zipfile.ZipFile(report_path) as archive:
            report=json.loads(archive.read('report.json'))
            snapshot=archive.read('snapshot.zip')
        assert report['identity']['firmwareSha256'] and len(report['identity']['bundleVersion']) == 64, report
        assert report['browser']['userAgent'] == await page.evaluate('navigator.userAgent')
        assert report['snapshot']['ticks'] <= stopped_ticks
        assert report['snapshot']['skipped'] == [], report['snapshot']
        assert report['inputs'] and all(type(event['guest_ticks']) is int for event in report['inputs'])
        assert 'Advance requires' in json.dumps(report['rawErrors'])
        assert report['trigger']['action'] == 'advance' and report['trigger']['options']['limit'] == 200001
        (out/'snapshot.zip').write_bytes(snapshot)
        with zipfile.ZipFile(out/'snapshot.zip') as archive:
            manifest=json.loads(archive.read('manifest.json'))
        assert manifest['schema'] == 'browser-snapshot-v1' and manifest['ram']['size'] > 0
        assert manifest['ram']['pages'] and len(manifest['architecture']) == 2
        assert await page.locator('#lcd-source').evaluate('c => c.toDataURL()') == pixels
        assert await page.evaluate('window.__state.virtual_ticks') == stopped_ticks
        await page.locator('#undo').click()
        await page.wait_for_function("document.querySelector('#live-label').textContent==='LIVE'")
        restored_ticks = await page.evaluate('window.__state.virtual_ticks')
        assert restored_ticks <= stopped_ticks - 120000000, (stopped_ticks, restored_ticks)
        await page.wait_for_function('(tick) => window.__state.virtual_ticks > tick', arg=restored_ticks)
        await page.screenshot(path=str(out / 'undo.png'))
        # Install the downloaded report's snapshot as the next browser-local
        # resume point. This proves that the actual ZIP can load in a fresh
        # matching Wasm engine, independently of in-memory Undo.
        await page.route('**/report-snapshot.zip',lambda route:route.fulfill(body=snapshot,content_type='application/zip'))
        await page.route('**/report-restore.html',lambda route:route.fulfill(body='<!doctype html><title>Snapshot restore gate</title>',content_type='text/html'))
        await page.goto(args.url.rstrip('/')+'/report-restore.html')
        await page.evaluate('''async(identity)=>{
          const digest=async bytes=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
          const bytes=await (await fetch('report-snapshot.zip')).arrayBuffer();
          const key=await digest(new TextEncoder().encode(JSON.stringify([1,identity.firmwareSha256,identity.bundleVersion,identity.storageProfileSha256])));
          const blob=new Blob([JSON.stringify({schema:1,bytes:bytes.byteLength,sha256:await digest(bytes)})+'\\n',bytes]);
          try {
            const dir=await (await navigator.storage.getDirectory()).getDirectoryHandle('fujimod-checkpoints-v1',{create:true});
            const file=await dir.getFileHandle(key,{create:true}),writer=await file.createWritable();
            await writer.write(blob);await writer.close();
          } catch {
            const data=await blob.arrayBuffer();
            await new Promise((resolve,reject)=>{
              const open=indexedDB.open('fujimod-checkpoints-v1',1);
              open.onsuccess=()=>{const db=open.result,tx=db.transaction('checkpoints','readwrite');
                tx.objectStore('checkpoints').put(data,key);tx.oncomplete=()=>{db.close();resolve();};tx.onerror=()=>reject(tx.error);};
              open.onerror=()=>reject(open.error);
            });
          }
        }''',report['identity'])
        await page.goto(args.url)
        if args.dat: await page.locator('#firmware-file').set_input_files(str(args.dat))
        await page.wait_for_function("document.querySelector('#live-label').textContent==='LIVE' || document.querySelector('#status').classList.contains('error')",timeout=180000)
        assert await page.locator('#live-label').inner_text()=='LIVE',await page.locator('#fault-raw').inner_text()
        await page.evaluate("window.__engineWorker.postMessage({type:'recovery_status'})")
        await page.wait_for_function('window.__cache?.mode === "resume"',timeout=30000)
        report_ticks=await page.evaluate('window.__ready.state.virtual_ticks')
        assert report['snapshot']['ticks']<=report_ticks<report['snapshot']['ticks']+12000000,(report['snapshot'],report_ticks)
        await page.wait_for_function('(tick)=>window.__state.virtual_ticks>tick',arg=report_ticks)
        await page.screenshot(path=str(out/'report-restored.png'))
        assert page_errors == [], page_errors
        result = dict(loads=results, before=before, stopped_ticks=stopped_ticks, restored_ticks=restored_ticks,
                      report_snapshot_ticks=report['snapshot']['ticks'],report_restored_ticks=report_ticks,
                      faults=await page.evaluate('window.__faults'), page_errors=page_errors)
        (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(dict(browser=name, loads=[r['seconds'] for r in results], stopped_ticks=stopped_ticks, restored_ticks=restored_ticks)), flush=True)
    except Exception:
        diagnostic=await page.evaluate('({faults:window.__faults, cache:window.__cache, ready:window.__ready, state:window.__state, status:document.querySelector("#status")?.textContent, details:document.querySelector("#fault-raw")?.textContent})')
        (out/'failure.json').write_text(json.dumps(dict(browser=diagnostic,page_errors=page_errors),indent=2)+'\n')
        await page.screenshot(path=str(out/'failure.png'))
        raise
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
