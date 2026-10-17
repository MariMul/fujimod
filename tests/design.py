"""Compare the built site with the approved HTML reference, in real browsers.

Requires a local preview with DAT auto-loading. Reference art is supplied
explicitly, so the public repository does not depend on a private checkout.
All screenshots and measured rectangles go to a fresh output directory.
"""
import argparse
import asyncio
import json
from pathlib import Path

from PIL import Image
from playwright.async_api import async_playwright


METRICS = """() => {
 const box=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right,bottom:r.bottom}};
 const controls={};for(const cap of document.querySelectorAll('.panel .cap')){
  const key=cap.textContent.trim(),e=cap.parentElement.querySelector('.dial,.seg,.slide,.key,.knurl');
  if(e&&key!=='POWER-ON COMBOS')controls[key]=box(e);
 }
 const all=s=>[...document.querySelectorAll(s)].map(box);
 return {header:box(document.querySelector('header')),logo:box(document.querySelector('header img')),
 stage:box(document.querySelector('.stage')),viewer:box(document.querySelector('.viewer')),
 table:box(document.querySelector('.gens')),footer:box(document.querySelector('footer')),
 ...(document.querySelector('.stage.idle') ? {drop:box(document.querySelector('.dropbox')),card:box(document.querySelector('.cams .body'))} : {
 panel:box(document.querySelector('.panel')),controls,
 disp:box(document.querySelector('.c2>.pair:last-child>.key:first-child')),
 keys:all('.panel .key'),dpad:box(document.querySelector('.dpad')),
 rows:all('.tree .r'),combos:all('.combo'),cardHeader:box(document.querySelector('.sd .head'))})};
}"""


def compare(reference, built, *, idle):
    """Geometry is in CSS pixels, independently of screenshot pixel density."""
    checks = []

    def same(label, a, b):
        delta = max(abs(a[k] - b[k]) for k in a)
        checks.append(dict(check=label, passed=delta <= .1,
                           maximum_delta_px=delta, reference=a, built=b))

    for key in ('header', 'logo', 'stage', 'viewer', 'table', 'footer'):
        same(key, reference[key], built[key])
    if idle:
        for key in ('drop', 'card'):
            same(key, reference[key], built[key])
        return checks
    same('panel', reference['panel'], built['panel'])
    for key, rect in reference['controls'].items():
        same(f'control {key}', rect, built['controls'][key])
    for key in ('disp', 'dpad', 'cardHeader'):
        same(key, reference[key], built[key])
    for key in ('keys', 'combos'):
        assert len(reference[key]) == len(built[key]), (key, reference[key], built[key])
        for i, rect in enumerate(reference[key]):
            same(f'{key} {i}', rect, built[key][i])
    # Tree contents come from the real FAT card. The mockup's names and sizes
    # are examples; the row geometry remains exact regardless of file count.
    for i, row in enumerate(built['rows']):
        checks.append(dict(check=f'card row {i}: 30 px', passed=row['height'] == 30,
                           height_px=row['height']))
    controls = built['controls']
    checks.append(dict(check='SCREEN bottom equals DISP/BACK bottom',
                       passed=abs(controls['SCREEN']['bottom']-built['disp']['bottom']) <= .1,
                       screen_bottom_px=controls['SCREEN']['bottom'],
                       disp_bottom_px=built['disp']['bottom']))
    panel = built['panel']
    min_inset = min(min(r['x']-panel['x'], panel['right']-r['right'])
                    for r in [*controls.values(), *built['keys'], *built['combos'], built['cardHeader']])
    checks.append(dict(check='panel horizontal inset >= 18 px', passed=min_inset >= 18,
                       minimum_inset_px=min_inset))
    checks.append(dict(check='40 px keys are circles',
                       passed=all(abs(r['width']-40)<.1 and abs(r['height']-40)<.1 for r in built['keys']),
                       sizes_px=[[r['width'],r['height']] for r in built['keys']]))
    return checks


def composite(reference, built, output):
    with Image.open(reference) as a, Image.open(built) as b:
        canvas = Image.new('RGB', (a.width+b.width, max(a.height,b.height)), (28,28,30))
        canvas.paste(a,(0,0)); canvas.paste(b,(a.width,0)); canvas.save(output)


async def capture(page, out, name, state, side):
    path = out/f'{name}-{side}-{state}.png'
    await page.screenshot(path=path, full_page=True)
    await page.screenshot(path=out/f'{name}-{side}-{state}-viewport.png')
    return path


async def check(p, args, name, kind):
    browser = await getattr(p,kind).launch(headless=False)
    options = dict(viewport=dict(width=1300,height=900),device_scale_factor=1)
    if name == 'phone':
        options = {**p.devices['iPhone 13'], 'viewport':dict(width=390,height=844), 'device_scale_factor':2}
    context = await browser.new_context(**options)
    await context.add_init_script("""const W=window.Worker;window.Worker=class extends W{
     constructor(...args){super(...args);if(String(args[0]).includes('pyodide-worker'))window.__worker=this;}};""")
    checks = []
    errors = []
    try:
        for state in ('idle','running','fault'):
            ref = await context.new_page()
            await ref.goto(args.reference.resolve().as_uri())
            await ref.evaluate("""(state)=>{
              document.querySelector('.variants').remove();
              const stages=document.querySelectorAll('.stage');
              stages[state==='idle'?1:0].remove();
              if(state!=='idle'){
                document.querySelector('.err').hidden=state!=='fault';
                document.querySelector('.bomb').style.display=state==='fault'?'':'none';
              }
            }""", state)
            if state == 'idle':
                page = await context.new_page()
                await page.route('**/local/firmware.dat',lambda r:r.fulfill(status=404,body=''))
                page.on('pageerror',lambda e:errors.append(str(e)))
                await page.goto(args.url)
                await page.wait_for_function('document.querySelector("#model-credit").textContent.includes("McCLOUD")')
                await page.wait_for_timeout(500)
            elif state == 'running':
                await page.close()
                page = await context.new_page()
                page.on('pageerror',lambda e:errors.append(str(e)))
                await page.goto(args.url)
                await page.wait_for_function('document.querySelector("#live-label").textContent==="LIVE" || document.querySelector("#status").classList.contains("error")',timeout=180000)
                assert await page.locator('#live-label').inner_text() == 'LIVE', await page.locator('#fault-raw').inner_text()
                await page.wait_for_timeout(600)
            else:
                await page.evaluate("""()=>__worker.dispatchEvent(new MessageEvent('message',{data:{type:'error',fault:{
                 kind:'unsupported-device',message:'Unsupported camera hardware',details:{type:'UnsupportedDevice',
                 device:'serial audio controller',operation:'write',address:'0xfffa000c',value:'0x00000001',
                 pc:'0x0094e2c4',cpu:0,error:'Unmodeled serial audio operation'}}}}))""")
                await page.wait_for_function('document.querySelector("#live-label").textContent==="STOPPED"')
            await page.evaluate('window.scrollTo(0,0)')
            rm, bm = await ref.evaluate(METRICS), await page.evaluate(METRICS)
            # Fault panels float over the page: all underlying geometry must
            # equal the running reference, while raw diagnostics vary at runtime.
            checks.extend(dict(state=state,**item) for item in compare(rm,bm,idle=state=='idle'))
            a = await capture(ref,args.output,name,state,'reference')
            b = await capture(page,args.output,name,state,'built')
            composite(a,b,args.output/f'{name}-{state}-composite.png')
            await ref.close()
        checks.append(dict(check='no browser exceptions',passed=not errors,errors=errors))
        (args.output/f'{name}-measurements.json').write_text(json.dumps(checks,indent=2)+'\n')
        failures = [c for c in checks if not c['passed']]
        print(json.dumps(dict(viewport=name,checks=len(checks),failures=failures)),flush=True)
        return not failures
    finally:
        await browser.close()


async def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url',default='http://127.0.0.1:5190/')
    parser.add_argument('--reference',type=Path,required=True,help='Approved mockup.html')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=False)
    async with async_playwright() as p:
        results=[await check(p,args,'desktop','chromium'),await check(p,args,'phone','webkit')]
    assert all(results), 'Geometry failed; inspect measurements and composites.'


if __name__=='__main__':
    asyncio.run(main())
