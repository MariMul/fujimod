"""Check dial motion in real browsers without a DAT or running emulator.

Run against Vite: python src/ui/dial.browser.py --url http://127.0.0.1:5190/
--output build/dial-proof. Output must remain in an ignored build directory.
"""
import argparse
import asyncio
import json
from pathlib import Path

from playwright.async_api import async_playwright

FIXTURE = '''<!doctype html><link rel="stylesheet" href="/src/style.css">
<div id="controls" style="width:240px;padding:20px"></div>
<script type="module">
import { InputPanel } from '/src/ui/controls.ts';
window.requests = [];
const viewer = new Proxy({}, {get: () => () => {}});
window.panel = new InputPanel(document.querySelector('#controls'), viewer,
  error => { throw error; }, async () => {});
panel.render([
  {id:'aperture', label:'Aperture', kind:'ring', supported:true,
    positions:['2.8','4','5.6','8','11','16'].map(id => ({id, label:id})), initial:'5.6'},
  {id:'command', label:'Command', kind:'dial', supported:true, relative:true},
], {setPosition: async (id, value) => requests.push({id,value})}, {
  layout:[{kind:'control',id:'aperture',variant:'dial'},
    {kind:'control',id:'command',variant:'ticks'}], combos:[]
});
window.fixtureReady = true;
</script>'''


async def paint(page):
    await page.evaluate('() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')


async def settled(dial):
    await dial.evaluate('''async node => {
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const values = node.querySelector('.vals');
      await new Promise(resolve => {
        const check = () => {
          const style = getComputedStyle(values);
          const done = node.classList.contains('ticks') ?
            Math.abs(parseFloat(style.backgroundPositionX) - parseFloat(values.style.backgroundPositionX)) < .01 :
            Math.abs(new DOMMatrixReadOnly(style.transform).m41 - new DOMMatrixReadOnly(values.style.transform).m41) < .01;
          if (done) resolve(); else requestAnimationFrame(check);
        }; check();
      });
    }''')


async def snapshot(dial):
    return await dial.evaluate('''node => {
      const values = node.querySelector('.vals');
      const on = values.querySelector('.on');
      return {x:new DOMMatrixReadOnly(getComputedStyle(values).transform).m41,
        ticks:parseFloat(getComputedStyle(values).backgroundPositionX),
        selected:node.getAttribute('aria-valuetext'),
        error:on ? on.getBoundingClientRect().x + on.offsetWidth/2 -
          (node.getBoundingClientRect().x + node.clientWidth/2) : null};
    }''')


async def check(browser_type, url, output):
    browser = await browser_type.launch()
    page = await browser.new_page(viewport=dict(width=480, height=400), has_touch=True)
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    await page.route('**/dial-test.html', lambda route: route.fulfill(body=FIXTURE, content_type='text/html'))
    await page.goto(url.rstrip('/') + '/dial-test.html')
    await page.wait_for_function('window.fixtureReady')
    dial = page.locator('.dial[data-id="aperture"]')
    await page.wait_for_timeout(80)
    initial = await snapshot(dial)
    box = await dial.bounding_box()
    x, y = box['x'] + box['width']/2, box['y'] + box['height']/2
    await page.mouse.move(x, y)
    await page.mouse.down()
    samples = []
    for dx in [4, 8, 12, 16, 20]:
        await page.mouse.move(x+dx, y)
        await paint(page)
        samples.append(await snapshot(dial))
    assert all(a['x'] > b['x'] for a,b in zip([initial]+samples, samples)), samples
    assert [s['selected'] for s in samples] == ['5.6','5.6','5.6','8','8'], samples
    distances = [a['x']-b['x'] for a,b in zip([initial]+samples[:3],samples[:4])]
    assert max(distances)-min(distances) < 1, distances
    assert await page.evaluate('requests') == [dict(id='aperture',value='8')]
    await page.screenshot(path=str(output/'held.png'))
    await page.mouse.up()
    await settled(dial)
    released = await snapshot(dial)
    assert abs(released['error']) < 1, released
    assert released['x'] > samples[-1]['x'], (released, samples[-1])
    # Reversal and cancellation restore the selected detent without extra input.
    await page.mouse.move(x, y)
    await page.mouse.down()
    await page.mouse.move(x-8, y)
    await paint(page)
    reverse = await snapshot(dial)
    assert reverse['x'] > released['x'], reverse
    await dial.dispatch_event('pointercancel', dict(pointerId=1))
    await page.mouse.up()
    await settled(dial)
    assert abs((await snapshot(dial))['error']) < 1
    assert await page.evaluate('requests.length') == 1
    # Keyboard remains discrete and snaps to its newly selected value.
    await dial.press('ArrowRight')
    await settled(dial)
    assert (await snapshot(dial))['selected'] == '11'
    assert abs((await snapshot(dial))['error']) < 1
    # A native touch start with matching fractional moves exercises touch capture.
    await dial.evaluate('''node => node.addEventListener('pointerdown', event => {
      window.touch = {trusted:event.isTrusted, type:event.pointerType};
      queueMicrotask(() => window.dispatchEvent(new PointerEvent('pointermove', {
        pointerId:event.pointerId,clientX:event.clientX+8,clientY:event.clientY,buttons:1
      })));
    }, {once:true})''')
    await dial.tap()
    await settled(dial)
    assert await page.evaluate('touch') == dict(trusted=True,type='touch')
    assert (await snapshot(dial))['selected'] == '11'
    # Relative tick strips retain their position across gestures and snap too.
    ticks = page.locator('.dial[data-id="command"]')
    box = await ticks.bounding_box()
    x, y = box['x']+box['width']/2, box['y']+box['height']/2
    for end in [16,32]:
        await page.mouse.move(x,y)
        await page.mouse.down()
        await page.mouse.move(x+20,y)
        await paint(page)
        assert abs((await snapshot(ticks))['ticks'] - (end+4)) < .01, await snapshot(ticks)
        await page.mouse.up()
        await settled(ticks)
        assert abs((await snapshot(ticks))['ticks'] - end) < .01, await snapshot(ticks)
    await page.screenshot(path=str(output/'released.png'))
    assert not errors, errors
    (output/'result.json').write_text(json.dumps(dict(initial=initial, samples=samples, released=released,
        requests=await page.evaluate('requests')), indent=2)+'\n')
    await browser.close()


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', default='http://127.0.0.1:5190/')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    async with async_playwright() as playwright:
        for name in ['chromium','webkit']:
            output = args.output/name
            output.mkdir(parents=True, exist_ok=False)
            await check(getattr(playwright,name), args.url, output)
            print(f'{name}: smooth drag, release snap, reversal, cancellation, keyboard and touch passed')


if __name__ == '__main__':
    asyncio.run(main())
