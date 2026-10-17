"""Real DAT + WebKit iPhone rotary gate; outputs stay outside tracked source.

Run with Playwright installed, against the Vite dev server with local runtime
mounts, e.g. uv run --with playwright python src/viewer/rotary.browser.py
--url http://127.0.0.1:5194/ --output /fresh/ignored/proof.

Playwright WebKit exposes native touch taps, but no native touch-move API. Each
swipe starts with touchscreen.tap (trusted pointerType=touch, real capture); the
probe injects matching touch pointer moves inside that down handler, before the
native release. This tests viewer/OrbitControls arbitration in WebKit, not iPhone
hardware panning, vibration, audio output or the physical mute switch.
"""
import argparse
import asyncio
import base64
import json
from html import escape
from pathlib import Path
from playwright.async_api import async_playwright

HELPERS = r"""() => {
  const v = window.__viewer, canvas = v.renderer.domElement;
  window.__pose = () => ({ position: v.camera.position.toArray(), quaternion: v.camera.quaternion.toArray() });
  window.__view = async (front, rise = 2, side = 0) => {
    v.faceScreen(); await new Promise(resolve=>setTimeout(resolve,400)); const d = v.home.z - v.homeTarget.z;
    v.camera.position.set(v.home.x + side, v.home.y + rise, v.homeTarget.z + (front ? -d : d));
    v.orbit.update(); v.camera.updateMatrixWorld(); v.root.updateMatrixWorld(true);
    v.dirty = v.leadersDirty = true;
  };
  window.__find = id => {
    const target = v.targets.get(id), rect = canvas.getBoundingClientRect();
    let found;
    target.traverse(mesh => {
      if (found || !mesh.geometry) return;
      const geometry = mesh.geometry, positions = geometry.attributes.position, indices = geometry.index;
      const count = (indices?.count ?? positions.count) / 3;
      const a = mesh.position.clone(), b = a.clone(), c = a.clone();
      for (let sample = 0; sample < Math.min(count, 96); sample++) {
        const i = Math.floor(sample * count / Math.min(count, 96)) * 3;
        a.fromBufferAttribute(positions, indices ? indices.getX(i) : i);
        b.fromBufferAttribute(positions, indices ? indices.getX(i+1) : i+1);
        c.fromBufferAttribute(positions, indices ? indices.getX(i+2) : i+2);
        a.add(b).add(c).multiplyScalar(1/3); mesh.localToWorld(a); a.project(v.camera);
        const x = rect.left + (a.x + 1) * rect.width / 2, y = rect.top + (1 - a.y) * rect.height / 2;
        if (x <= rect.left || x >= rect.right || y <= rect.top || y >= rect.bottom) continue;
        const hit = v.pick({clientX:x, clientY:y});
        if (hit && v.rotary.get(hit.id) === v.rotary.get(id)) {
          const rx = Math.round(x), ry = Math.round(y);
          if (![[0,0],[-1,0],[1,0],[0,-1],[0,1]].every(([dx,dy])=>v.rotary.get(v.pick({clientX:rx+dx,clientY:ry+dy})?.id)===v.rotary.get(id))) continue;
          found = {x:rx,y:ry}; break;
        }
      }
    });
    if (!found) throw Error(`No visible touch point for ${id}`);
    return found;
  };
  window.addEventListener('pointerdown', event => {window.__down={x:event.clientX,y:event.clientY,hit:v.pick(event)};}, true);
  window.__feedback = 0; window.__soundClicks = 0;
  const step = v.detent.step.bind(v.detent);
  v.detent.step = () => {
    const context = v.detent.context;
    if (context && !context.__observed) {
      const create = context.createOscillator.bind(context);
      context.createOscillator = () => {const node=create(), start=node.start.bind(node);node.start=(...args)=>{window.__soundClicks++;start(...args);};return node;};
      context.__observed = true;
    }
    window.__feedback++; step();
  };
  window.__swipe = (dx, dy = 0, cancel = false) => {
    window.__afterDown = event => {
      window.__touch = {trusted:event.isTrusted, type:event.pointerType, captured:canvas.hasPointerCapture(event.pointerId)};
      for (let i = 1; i <= 8; i++) canvas.dispatchEvent(new PointerEvent('pointermove', {
        pointerId:event.pointerId, pointerType:event.pointerType, isPrimary:event.isPrimary,
        clientX:event.clientX + dx*i/8, clientY:event.clientY + dy*i/8, buttons:1, bubbles:true, cancelable:true
      }));
      if (cancel) canvas.dispatchEvent(new PointerEvent('pointercancel', {pointerId:event.pointerId, pointerType:'touch', bubbles:true}));
      window.__afterDown = undefined;
    };
  };
  window.__select = (id, value) => {
    const dial=document.querySelector(`.dial[data-id="${id}"]`);
    if(dial){const values=[...dial.querySelectorAll('.vals span')].map(n=>n.dataset.value),target=values.indexOf(value),current=Number(dial.getAttribute('aria-valuenow'));if(target<0)throw Error('Unknown dial value');for(let i=0;i<Math.abs(target-current);i++)dial.dispatchEvent(new KeyboardEvent('keydown',{key:target>current?'ArrowRight':'ArrowLeft',bubbles:true}));}
    else document.querySelector(`.control-node[data-id="${id}"] button[data-value="${value}"]`).click();
  };
  window.__lcdHash = async (region = 'values') => {
    // Shooting values and focus scale; excludes the clock at the top.
    const data = region === 'menu' ? v.lcd.getContext('2d').getImageData(0,0,v.lcd.width,v.lcd.height).data :
      v.lcd.getContext('2d').getImageData(0, v.lcd.height*.85, v.lcd.width, v.lcd.height*.15).data;
    return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', data))).map(x=>x.toString(16).padStart(2,'0')).join('');
  };
}"""


async def lcd(page, path):
    encoded = await page.locator('#lcd-source').evaluate("c=>c.toDataURL('image/png').split(',')[1]")
    path.write_bytes(base64.b64decode(encoded))


async def settle(page):
    await page.evaluate('async()=>{await __engine.queue;window.__settleTick=__engine.state.virtual_ticks+24000000;}')
    await page.wait_for_function('__engine.state.virtual_ticks>=__settleTick',timeout=90000)
    await page.wait_for_timeout(750)


def write_report(output, report):
    (output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    sections=[]
    for check in report['checks']:
        name=check['name']
        figures=''.join(f'<figure><figcaption>{phase.title()}</figcaption><a href="{name}-{phase}.png">'
                        f'<img src="{name}-lcd-{phase}.png" alt="{escape(name)} LCD {phase}">'
                        f'<img src="{name}-{phase}.png" alt="{escape(name)} model {phase}"></a></figure>'
                        for phase in ['before','after'])
        sections.append(f'<section><h2>{escape(name)} · {check["detents"]} detents</h2><div>{figures}</div></section>')
    (output/'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Rotary touch proof</title>'
        '<style>body{background:#151515;color:#eee;font:16px system-ui;max-width:1100px;margin:40px auto;padding:20px}'
        'section>div{display:grid;grid-template-columns:1fr 1fr;gap:20px}figure{margin:0}img{width:100%;display:block;margin:10px 0}'
        'a{color:inherit}section{border-top:1px solid #666;margin-top:32px}</style><h1>WebKit iPhone 13 rotary proof</h1>'
        '<p>Real local firmware. Trusted native touch starts and injected touch pointer moves. '
        'Physical iPhone audio, vibration and mute switch were not measured.</p>'+''.join(sections))


async def run(args):
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as p:
        browser = await p.webkit.launch(headless=False)
        page = await browser.new_page(**p.devices['iPhone 13'])
        errors = []
        page.on('pageerror', lambda e: errors.append(str(e)))
        # Disable hot reload only, while preserving Vite's real environment/CSS.
        await page.add_init_script("window.WebSocket=class{static OPEN=1;static CONNECTING=0;readyState=0;addEventListener(){}removeEventListener(){}send(){}close(){}};")
        await page.add_init_script("""window.__requests=[];
          const W=window.Worker;window.Worker=class extends W{
            constructor(...a){super(...a);this.addEventListener('message',({data})=>{if(data.state?.display)window.__state=data.state;});}
            postMessage(data,...a){if(data.type==='request'&&data.action!=='advance'&&data.action!=='state')window.__requests.push(data);return super.postMessage(data,...a);}
          };""")
        async def expose(route):
            response = await route.fetch()
            body = (await response.text()).replace('this.host = host;', 'window.__viewer=this;this.host=host;')
            body = body.replace('this.detent.beginGesture(event);', 'this.detent.beginGesture(event);window.__afterDown?.(event);')
            await route.fulfill(response=response, body=body)
        await page.route('**/src/viewer/model.ts*', expose)
        async def expose_engine(route):
            response=await route.fetch()
            body=(await response.text()).replace('this.onError = onError;', 'window.__engine=this;this.onError=onError;')
            await route.fulfill(response=response,body=body)
        await page.route('**/src/engine/worker-engine.ts*', expose_engine)
        report = dict(browser=browser.version, emulation='iPhone 13', touch_moves='injected after trusted native touch down', checks=[])
        try:
            await page.goto(args.url)
            if args.dat:
                await page.wait_for_function("document.querySelector('#camera-table').children.length > 0")
                await page.locator('#firmware-file').set_input_files(str(args.dat))
            await page.wait_for_function("document.querySelector('#live-label').textContent==='LIVE'", timeout=180000)
            await page.wait_for_function("Number(document.querySelector('#viewer').dataset.frames)>3", timeout=60000)
            await page.evaluate(HELPERS)
            await page.evaluate('()=>__engine.setScene({width:160,height:120,rgb:new Uint8Array(160*120*3).fill(128)})')
            await page.locator('button[data-id=disp]').tap()
            await page.evaluate('window.scrollTo(0,0)')
            await settle(page)
            print('Booted real firmware in WebKit', flush=True)

            async def check(name, model_id, dx, dy=0, front=False, setup=None, lcd_region='values'):
                if setup: await page.evaluate(setup)
                await settle(page)
                await page.evaluate('(front)=>__view(front,front?1.4:2,front?1.2:0)', front)
                await page.wait_for_timeout(100)
                point = await page.evaluate('(id)=>__find(id)', model_id)
                # A trusted stationary touch prepares audio without taking a detent.
                await page.touchscreen.tap(point['x'],point['y'])
                await page.wait_for_timeout(100)
                before = await page.evaluate('(id)=>({pose:__pose(),rotation:__viewer.rotary.get(id).part.pivot.quaternion.toArray(),feedback:__feedback,sounds:__soundClicks,requests:__requests.length,queue:__state.navigation.physical_input_queue})', model_id)
                before_hash = await page.evaluate('(region)=>__lcdHash(region)', lcd_region)
                await page.screenshot(path=str(args.output/f'{name}-before.png'))
                await lcd(page,args.output/f'{name}-lcd-before.png')
                await page.evaluate('([dx,dy])=>__swipe(dx,dy)', [dx,dy])
                await page.touchscreen.tap(point['x'],point['y'])
                await settle(page)
                after = await page.evaluate('(id)=>({pose:__pose(),rotation:__viewer.rotary.get(id).part.pivot.quaternion.toArray(),feedback:__feedback,sounds:__soundClicks,audioState:__viewer.detent.context?.state,touch:__touch,down:__down,requests:__requests,queue:__state.navigation.physical_input_queue})', model_id)
                after_hash = await page.evaluate('(region)=>__lcdHash(region)', lcd_region)
                await page.screenshot(path=str(args.output/f'{name}-after.png'))
                await lcd(page,args.output/f'{name}-lcd-after.png')
                item=dict(name=name,point=point,down=after['down'],pose=before['pose'],rotation_before=before['rotation'],rotation_after=after['rotation'],detents=after['feedback']-before['feedback'],sound_clicks=after['sounds']-before['sounds'],audio_state=after.get('audioState'),lcd_before=before_hash,lcd_after=after_hash,queue_before=before['queue'],queue_after=after['queue'],requests=after['requests'][before['requests']:])
                report['checks'].append(item)
                write_report(args.output,report)
                print(json.dumps(item),flush=True)
                assert after['pose']==before['pose'], f'{name} orbited'
                assert after['rotation']!=before['rotation'], f'{name} did not visibly rotate'
                assert after['feedback']-before['feedback']==max(abs(dx),abs(dy))//16, f'{name} lost a detent'
                assert after['touch']==dict(trusted=True,type='touch',captured=True), after['touch']
                assert after['sounds']-before['sounds']==after['feedback']-before['feedback'], f'{name} missing Web Audio clicks'
                assert before_hash!=after_hash, f'{name} did not change LCD shooting values'
                if name=='rear-command':
                    assert [request['action'] for request in item['requests']]==['press','release','press','release']
                    assert after['queue']['completed']-before['queue']['completed']==4

            await check('shutter-speed','shutter_speed',-32)
            await check('exposure','exposure_compensation',32,setup="__select('shutter_speed','A')")
            await check('aperture','aperture',32,front=True)
            # Assign ISO through the stock Fn7 menu, then turn AUTO1 -> AUTO2.
            await check('control-ring','control_ring',32,front=True,lcd_region='menu',setup="async()=>{for(const id of ['fn7','down','menu_ok']){await __engine.press(id);await __engine.release(id);}}")
            # Leave the ISO popup and use the stock Q custom-setting BASE -> C2.
            await check('rear-command','command_push',32,lcd_region='menu',setup="async()=>{for(const id of ['disp','q']){await __engine.press(id);await __engine.release(id);}}")
            # A touch starting on empty canvas must continue to OrbitControls.
            await page.evaluate("""() => {
              const c=__viewer.renderer.domElement;
              c.addEventListener('pointerdown',e=>{for(let i=1;i<=8;i++)c.dispatchEvent(new PointerEvent('pointermove',{pointerId:e.pointerId,pointerType:'touch',clientX:e.clientX+5*i,clientY:e.clientY,buttons:1,bubbles:true}));},{once:true});
            }""")
            rect = await page.locator('#viewer canvas').first.bounding_box()
            before = await page.evaluate('__pose()')
            await page.touchscreen.tap(rect['x']+10,rect['y']+50)
            assert await page.evaluate('__pose()') != before, 'Empty touch failed to orbit'
            report['empty_touch_orbits']=True
            assert not errors, errors
            report['errors']=errors
            write_report(args.output,report)
        except BaseException:
            await page.screenshot(path=str(args.output/'failed.png'),full_page=True)
            (args.output/'errors.json').write_text(json.dumps(errors,indent=2))
            raise
        finally:
            await browser.close()


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--url',default='http://127.0.0.1:5194/')
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--dat',type=Path)
    asyncio.run(run(parser.parse_args()))
