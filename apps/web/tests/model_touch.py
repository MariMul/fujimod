"""Real X70 model picking and phone pointer delivery, without firmware inputs.

Run against a Vite development server. The page uses the real model and panel
with an input-recording engine; it never starts an emulator or accesses a camera.
"""
import argparse
import asyncio
import json
from pathlib import Path

from playwright.async_api import async_playwright


HTML = '''<meta name="viewport" content="width=device-width,initial-scale=1">
<div id="workspace"><div id="viewer" class="viewer"></div>
<div id="panel"><div id="controls"></div></div></div>'''

SETUP = '''async () => {
  await import('/src/style.css');
  const THREE = await import('/node_modules/three/build/three.module.js');
  const {ModelViewer} = await import('/src/viewer/model.ts');
  const {InputPanel} = await import('/src/ui/controls.ts');
  const {model, panel} = await import('/devices/x70/index.ts');
  const host = document.querySelector('#viewer');
  host.style.cssText = 'height:440px;min-height:0;width:100%';
  const viewer = window.testViewer = new ModelViewer(host,
    document.createElementNS('http://www.w3.org/2000/svg','svg'), document.querySelector('#workspace'));
  await viewer.load(model);
  window.inputEvents = [];
  const engine = {
    press: async id => inputEvents.push(['press', id]),
    release: async id => inputEvents.push(['release', id]),
    setPosition: async () => {},
  };
  const controls = window.testPanel = new InputPanel(document.querySelector('#controls'), viewer,
    error => {throw error}, async () => {});
  controls.render(['up','down','left','right','menu_ok','q','disp','ae_af_lock']
    .map(id => ({id, label:id, kind:'button', supported:true})), engine, panel);
  // Locate the visible spokes independently of the viewer's picking helpers.
  window.controlPoint = (name, x=.5, y=.5) => {
    const node=viewer.root.getObjectByName(name), box=new THREE.Box3().setFromObject(node);
    const point=new THREE.Vector3(box.min.x+(box.max.x-box.min.x)*x,
      box.min.y+(box.max.y-box.min.y)*y,box.max.z).project(viewer.camera);
    const r=viewer.renderer.domElement.getBoundingClientRect();
    return {x:r.left+(point.x*.5+.5)*r.width,y:r.top+(-point.y*.5+.5)*r.height};
  };
  window.wholePadGlows = () => {
    let glows = false;
    for(const name of ['menu_sw_49','menu_frame_48'])viewer.root.getObjectByName(name).traverse(node=>{
      for(const material of (Array.isArray(node.material)?node.material:[node.material]))
        if(material?.emissive && material.userData.baseEmissive
          && !material.emissive.equals(material.userData.baseEmissive))glows=true;
    });
    return glows;
  };
  await new Promise(requestAnimationFrame);
  await new Promise(requestAnimationFrame);
}'''


async def check(args):
    args.output.mkdir(parents=True, exist_ok=False)
    origin = args.url.rstrip('/')
    async with async_playwright() as p:
        browser = await p.webkit.launch()
        page = await browser.new_page(**p.devices['iPhone 13'])
        errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        await page.route(origin + '/', lambda route: route.fulfill(body=HTML, content_type='text/html'))
        await page.goto(origin + '/')
        await page.evaluate(SETUP)
        await page.wait_for_timeout(100)
        receipts = []

        async def tap(identifier, point):
            before = await page.evaluate('inputEvents.length')
            await page.touchscreen.tap(point['x'], point['y'])
            await page.wait_for_function('n=>inputEvents.length>=n+2', arg=before)
            events = await page.evaluate('n=>inputEvents.slice(n)', before)
            assert events == [['press', identifier], ['release', identifier]], events
            feedback = page.locator('.model-input-feedback')
            assert await feedback.is_visible()
            assert await feedback.get_attribute('data-input') == identifier
            if identifier in ('up', 'down', 'left', 'right'):
                assert await feedback.inner_text() == ''
                assert await feedback.locator('svg').count() == 1
                assert await feedback.get_attribute('aria-label') == identifier.title()
            await page.wait_for_function('id=>document.querySelector(".model-input-marker").dataset.input===id', arg=identifier)
            assert not await page.evaluate('wholePadGlows()')
            receipts.append(dict(id=identifier, point=point, events=events))

        for identifier, x, y in [('menu_ok', .5, .5), ('up', .5, .81), ('down', .5, .19),
                                  ('left', .19, .5), ('right', .81, .5)]:
            point = await page.evaluate('p=>controlPoint("menu_sw_49",p.x,p.y)', dict(x=x, y=y))
            await tap(identifier, point)
            if identifier == 'down':
                await page.locator('#viewer').screenshot(path=str(args.output/'down.png'))
        # A slightly missed Q tap still reaches Q, rather than orbiting.
        point = await page.evaluate('()=>controlPoint("LBL_Q_6")')
        point['x'] += 14
        await tap('q', point)
        await page.wait_for_timeout(1250)
        assert await page.locator('.model-input-feedback').is_hidden()
        assert await page.locator('.model-input-marker').is_hidden()
        # A genuine screen contact keeps its coordinates, even near a button.
        screen = await page.evaluate('''() => {
          const point=controlPoint('monitorr_screen_46',.99,.5);
          return testViewer.pick({...point,clientX:point.x,clientY:point.y,pointerType:'touch'});
        }''')
        assert screen['id'] == 'lcd_touch' and 'point' in screen, screen
        await page.evaluate('testViewer.faceLens()')
        await page.wait_for_timeout(450)
        hidden = await page.evaluate('''() => {
          const p=controlPoint('LBL_Q_6');
          return testViewer.nearbyButton({clientX:p.x,clientY:p.y,pointerType:'touch'});
        }''')
        assert hidden is None or hidden['id'] != 'q', hidden
        assert not errors, errors
        result = dict(browser=browser.version, taps=receipts, hidden_button=hidden, errors=errors)
        (args.output/'result.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result), flush=True)
        await browser.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', default='http://127.0.0.1:5193')
    parser.add_argument('--output', type=Path, required=True)
    asyncio.run(check(parser.parse_args()))


if __name__ == '__main__': main()
