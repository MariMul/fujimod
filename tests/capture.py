"""Built-site WebKit/iPhone boot, MENU, touch dial, card JPEG and Playback gate.

Observe the existing worker protocol; all camera actions and the card download
use the visible UI. No source routes, firmware patches or private engine hooks.
"""
import argparse
import asyncio
import base64
import hashlib
import json
from pathlib import Path
import time

from playwright.async_api import async_playwright

MENU_HEADER = 'e598e0535f6d69203cbd325edcedde2cd530f706f54e00442701481ce48610b8'


async def check(args):
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as p:
        browser = await p.webkit.launch(headless=False)
        page = await browser.new_page(**p.devices['iPhone 13'], ignore_https_errors=True)
        errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        await page.add_init_script('''(() => {
          window.__faults=[]; window.__requests=[];
          const Original=window.Worker;
          window.Worker=class extends Original {
            constructor(...args) { super(...args); this.addEventListener('message',({data})=>{
              if(data.state?.display) window.__state=data.state;
              if(data.runtime) window.__runtime=data.runtime;
              if(data.type==='error') window.__faults.push(data);
              if(data.type==='ready') window.__ready=data;
            }); }
            postMessage(data,...args) {
              if(data.type==='request'&&!['state','advance'].includes(data.action)) window.__requests.push(data);
              return super.postMessage(data,...args);
            }
          };
        })();''')
        report = dict(url=args.url, browser=browser.version, device='iPhone 13', checks={})

        async def wait(expression, timeout=180):
            deadline=time.monotonic()+timeout
            while time.monotonic()<deadline:
                faults=await page.evaluate('window.__faults')
                if faults: raise AssertionError(faults)
                if await page.evaluate(expression): return
                await asyncio.sleep(.25)
            raise TimeoutError(await page.evaluate('({status:document.querySelector("#status")?.textContent,state:window.__state})'))

        async def save(name):
            await page.locator('#viewer').scroll_into_view_if_needed()
            await page.wait_for_timeout(250)
            await page.screenshot(path=str(args.output/f'{name}.png'))
            png=await page.locator('#lcd-source').evaluate("c=>c.toDataURL('image/png').split(',')[1]")
            (args.output/f'{name}-lcd.png').write_bytes(base64.b64decode(png))
            (args.output/f'{name}-state.json').write_text(json.dumps(await page.evaluate('window.__state'),indent=2)+'\n')

        async def tap(locator):
            await locator.scroll_into_view_if_needed()
            await page.wait_for_timeout(250)
            await locator.tap()

        header = '''async()=>{
          const rgba=document.querySelector('#lcd-source').getContext('2d').getImageData(0,0,720,48).data;
          const rgb=new Uint8Array(720*48*3);
          for(let s=0,t=0;s<rgba.length;s+=4,t+=3){rgb[t]=rgba[s];rgb[t+1]=rgba[s+1];rgb[t+2]=rgba[s+2];}
          return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',rgb)),n=>n.toString(16).padStart(2,'0')).join('');
        }'''
        try:
            started=time.monotonic()
            await page.goto(args.url)
            if args.dat: await page.locator('#firmware-file').set_input_files(str(args.dat))
            await wait('document.querySelector("#live-label").textContent==="LIVE"')
            report['boot_seconds']=time.monotonic()-started
            await wait('Number(document.querySelector("#viewer").dataset.frames)>3')
            assert await page.locator('.control-row[data-id="battery_cover"] button').is_disabled()
            assert await page.locator('.control-row[data-id="battery_cover"] button').inner_text() == 'Not modelled'
            report['checks']['battery_cover_has_no_host_power_interlock']=True
            await tap(page.locator('#face-screen'))
            await save('shooting')
            print('BOOT',report['boot_seconds'],flush=True)

            # Count both actual source DMA generations and presented generations.
            before=await page.evaluate('({tick:__state.virtual_ticks,image:__state.image_frames,presented:cameraPerformance.runtime.presentedImageFrames,wall:performance.now()})')
            await page.wait_for_timeout(10000)
            after=await page.evaluate('({tick:__state.virtual_ticks,image:__state.image_frames,presented:cameraPerformance.runtime.presentedImageFrames,wall:performance.now()})')
            guest=(after['tick']-before['tick'])/12000000
            report['preview']=dict(speed=guest/((after['wall']-before['wall'])/1000),
                image_guest_fps=(after['image']-before['image'])/guest,
                presented_guest_fps=(after['presented']-before['presented'])/guest)
            assert report['preview']['image_guest_fps']>=9.5,report['preview']
            assert report['preview']['presented_guest_fps']>=9.5,report['preview']
            print('PREVIEW',report['preview'],flush=True)

            await tap(page.get_by_role('button',name='Menu / OK',exact=True))
            deadline=time.monotonic()+60
            while await page.evaluate(header)!=MENU_HEADER:
                if time.monotonic()>deadline: raise AssertionError('Exact English MENU header did not arrive')
                await wait('!window.__faults.length',timeout=1)
                await asyncio.sleep(.2)
            await save('menu'); report['checks']['english_menu_header_sha256']=MENU_HEADER
            await tap(page.get_by_role('button',name='Display / Back',exact=True))
            await wait('window.__state.navigation.physical_input_queue.completed>=4 && !window.__state.navigation.physical_inputs.held.length')
            deadline=time.monotonic()+60
            while await page.evaluate(header)==MENU_HEADER:
                if time.monotonic()>deadline: raise AssertionError('Shooting pixels did not return after BACK')
                await asyncio.sleep(.2)
            print('MENU / BACK',flush=True)

            dial=page.locator('.control-row[data-id="shutter_speed"] select')
            await dial.select_option('1/125')
            old=await dial.input_value()
            await tap(page.locator('.control-row[data-id="shutter_speed"] > span'))
            assert await dial.input_value()!=old,'Touching the dial label did not take a detent'
            await wait('window.__requests.some(r=>r.action==="set_position"&&r.options.id==="shutter_speed")')
            report['checks']['panel_dial_trusted_touch']=True
            await page.locator('.control-row[data-id="focus_mode"] select').select_option('M')
            await dial.select_option('1/125')
            await page.evaluate('window.__settleTick=__state.virtual_ticks+24000000')
            await wait('window.__state.virtual_ticks>=window.__settleTick')
            await tap(page.get_by_role('button',name='Shutter full',exact=True))
            print('FULL SHUTTER',flush=True)
            await wait('window.__state.capture?.completed===1',timeout=360)
            await save('capture')
            download=page.locator('#card-files button[aria-label$=".JPG"]')
            await download.wait_for(timeout=30000)
            async with page.expect_download(timeout=60000) as pending: await tap(download.first)
            file=await pending.value
            await file.save_as(args.output/'photo.jpg')
            photo=(args.output/'photo.jpg').read_bytes()
            assert photo.startswith(b'\xff\xd8') and photo.endswith(b'\xff\xd9')
            digest=hashlib.sha256(photo).hexdigest()
            report['photo']=dict(name=file.suggested_filename,bytes=len(photo),sha256=digest)
            await page.evaluate('(hash)=>window.__photoHash=hash',digest)
            print('CARD JPEG',report['photo'],flush=True)

            await tap(page.get_by_role('button',name='Playback',exact=True))
            await wait('''window.__state.ui?.mode===4 && window.__state.ui.pending===0 &&
              window.__state.devices?.some(d=>d.device==='JPEGDecodeControl'&&d.operations?.some(o=>o.source_sha256===window.__photoHash&&o.width===4896&&o.height===3264)) &&
              window.__state.display.last_frame.main?.source==='0x4318b00' ''',timeout=240)
            await page.evaluate('window.__playbackTick=__state.virtual_ticks')
            await wait('window.__state.virtual_ticks-window.__playbackTick>=60000000',timeout=180)
            await save('playback')
            report['checks']['saved_full_image_playback']=True
            report['runtime']=await page.evaluate('window.__runtime')
            report['errors']=errors
            assert not errors,errors
            print('PLAYBACK PASSED',flush=True)
        except BaseException:
            await page.screenshot(path=str(args.output/'failure.png'))
            report['failure']=await page.evaluate('({state:window.__state,faults:window.__faults,status:document.querySelector("#status")?.textContent})')
            raise
        finally:
            report['inputs']=await page.evaluate('window.__requests')
            (args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
            await browser.close()


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url',default='http://127.0.0.1:5190/')
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--dat',type=Path)
    asyncio.run(check(parser.parse_args()))
