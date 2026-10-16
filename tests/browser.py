"""Real local DAT boot, physical-input acknowledgement and download browser checks.

Requires Playwright's Python package and browsers. Output is caller-selected,
fresh and outside source; no hardware access. WebKit emulates iPhone 13 on macOS.
"""
import argparse
import asyncio
import base64
import hashlib
import io
import json
from pathlib import Path
import time
import zipfile
from playwright.async_api import async_playwright


async def check(playwright, args, engine):
    out = args.output / engine
    out.mkdir()
    browser = await getattr(playwright, engine).launch(headless=False)
    options = dict(playwright.devices['iPhone 13']) if engine == 'webkit' else dict(viewport=dict(width=1440, height=1080), device_scale_factor=1)
    page = await browser.new_page(**options)
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.on('console', lambda message: errors.append(message.text) if message.type == 'error' else None)
    await page.add_init_script("""(() => {
      const WorkerClass = window.Worker;
      window.__messages = []; window.__sensorAcks = 0;
      window.Worker = class extends WorkerClass {
        constructor(...args) { super(...args); this.addEventListener('message', ({data}) => {
          if(data.state?.display) window.__state = data.state;
          if(data.type === 'sensor_ready') window.__sensorAcks++;
          if(data.type === 'error') window.__messages.push(data);
        }); }
        postMessage(data, ...args) { if(data.type === 'request' && data.action !== 'advance' && data.action !== 'state') window.__messages.push(data); return super.postMessage(data,...args); }
      };
    })();""")
    start = time.perf_counter()
    try:
        await page.goto(args.url)
        if args.dat:
            await page.locator('#firmware-prompt').wait_for(state='visible')
            await page.screenshot(path=str(out/'empty.png'),full_page=True)
            # An unsupported upload leaves the neutral viewer usable.
            await page.locator('#firmware-file').set_input_files(dict(name='unknown.dat',mimeType='application/octet-stream',buffer=b'unsupported firmware fixture'))
            await page.wait_for_function("document.querySelector('#status').textContent.startsWith('Unsupported firmware')")
            assert await page.locator('#panel').is_hidden()
            # Fill a browser File with local bytes, then send it through the
            # viewer's actual drop handler (without firing a second picker boot).
            await page.locator('#firmware-file').evaluate('input => input.onchange = null')
            await page.locator('#firmware-file').set_input_files(str(args.dat))
            await page.locator('#firmware-file').evaluate('''input => {
              const transfer = new DataTransfer(); transfer.items.add(input.files[0]);
              document.querySelector('#viewer').dispatchEvent(new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }));
            }''')
        await page.wait_for_function("document.querySelector('#live-label').textContent === 'LIVE' || document.querySelector('#status').classList.contains('error')", timeout=180000)
        assert await page.locator('#live-label').inner_text() == 'LIVE', await page.locator('#status').inner_text()
        await page.wait_for_function("Number(document.querySelector('#viewer').dataset.frames) >= 5", timeout=60000)
        initial = await page.locator('#viewer').evaluate('(v) => ({...v.dataset})')
        await page.screenshot(path=str(out / 'page.png'), full_page=True)
        await page.locator('#viewer').screenshot(path=str(out / 'model.png'))
        lcd = await page.locator('#lcd-source').evaluate("c => c.toDataURL('image/png').split(',')[1]")
        raw = base64.b64decode(lcd)
        (out / 'lcd.png').write_bytes(raw)
        assert len(raw) > 10000, 'LCD is blank'
        # A fast panel tap must change the firmware LCD, and renders keep advancing.
        await (page.get_by_role('button', name='Menu / OK', exact=True).tap() if engine == 'webkit' else page.get_by_role('button', name='Menu / OK', exact=True).click())
        await page.wait_for_timeout(6000)
        (out/'input-state.json').write_text(json.dumps(await page.evaluate('({state:window.__state,messages:window.__messages})'),indent=2))
        after = await page.locator('#lcd-source').evaluate("c => c.toDataURL('image/png').split(',')[1]")
        assert after != lcd, 'Menu input did not change the firmware LCD'
        input_state = await page.evaluate('window.__state')
        assert input_state['navigation']['physical_input_queue']['completed'] >= 2
        assert input_state['navigation']['physical_inputs']['held'] == []
        assert input_state['navigation']['current_menu']['pointer'] != 0, 'Stock menu did not open'
        assert not await page.locator('#status').evaluate("e => e.classList.contains('error')"), await page.locator('#status').inner_text()
        final = await page.locator('#viewer').evaluate('(v) => ({...v.dataset})')
        assert int(final['frames']) > int(initial['frames'])
        assert int(final['renderFrames']) > int(initial['renderFrames'])
        await page.screenshot(path=str(out / 'menu.png'), full_page=True)
        await page.get_by_role('button', name='Display / Back', exact=True).click()
        await page.wait_for_timeout(1500)
        if args.scene:
            # Reset the default scene without opening any host webcam.
            await page.locator('#webcam').dispatch_event('change')
            await page.wait_for_function('window.__sensorAcks >= 1', timeout=30000)
            await page.locator('#image-file').set_input_files(str(args.scene))
            await page.wait_for_function('window.__sensorAcks >= 2', timeout=30000)
            # Off restores the chosen image without requesting any webcam.
            assert not await page.locator('#webcam').is_checked()
            await page.locator('#webcam').dispatch_event('change')
            await page.wait_for_function('window.__sensorAcks >= 3', timeout=30000)
            assert not await page.locator('#status').evaluate("e => e.classList.contains('error')")
        # Exercise download from the real patch worker, save and check the ZIP members.
        async with page.expect_download(timeout=60000) as pending:
            await page.get_by_role('button', name='Build SD files').click()
        download = await pending.value
        archive = out / download.suggested_filename
        await download.save_as(archive)
        with zipfile.ZipFile(archive) as package:
            assert set(package.namelist()) == {'ffboot.bin', 'ffsys.bin'}
            hashes = {name: hashlib.sha256(package.read(name)).hexdigest() for name in package.namelist()}
            if args.expected:
                for name, digest in hashes.items():
                    assert hashlib.sha256((args.expected / name).read_bytes()).hexdigest() == digest, name
        assert not errors, errors
        report = dict(engine=engine, browser=browser.version, emulation='iPhone 13' if engine=='webkit' else 'desktop',
                      boot_seconds=float(initial['bootSeconds']), elapsed_seconds=time.perf_counter()-start,
                      initial=initial, final=final, lcd_png_sha256=hashlib.sha256(raw).hexdigest(), output_sha256=hashes, errors=errors)
        (out / 'result.json').write_text(json.dumps(report, indent=2)+'\n')
        print(json.dumps(report), flush=True)
    except BaseException:
        await page.screenshot(path=str(out/'failed.png'), full_page=True)
        (out/'errors.json').write_text(json.dumps(errors,indent=2))
        raise
    finally:
        await browser.close()


async def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--url', default='http://127.0.0.1:5190/')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--expected', type=Path)
    parser.add_argument('--scene', type=Path, help='Also check image input and reset to the analytic scene')
    parser.add_argument('--dat', type=Path, help='Choose a local DAT instead of automatic dev boot')
    parser.add_argument('--engine', choices=['chromium','webkit','both'], default='both')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as playwright:
        for engine in ['chromium','webkit'] if args.engine=='both' else [args.engine]:
            await check(playwright, args, engine)


if __name__ == '__main__':
    asyncio.run(main())
