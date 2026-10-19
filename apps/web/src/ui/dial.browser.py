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
<script type="module" src="/src/ui/dial.fixture.tsx"></script>'''


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
    page.on('pageerror', lambda error: (errors.append(str(error)), print(str(error), flush=True)))
    await page.route('**/dial-test.html', lambda route: route.fulfill(body=FIXTURE, content_type='text/html'))
    await page.goto(url.rstrip('/') + '/dial-test.html')
    await page.wait_for_function('window.fixtureReady')
    results = {}
    for identifier, initial_value, preceding in [('aperture','5.6','4'), ('shutter_speed','125','60')]:
        dial = page.locator(f'.dial[data-id="{identifier}"]')
        await paint(page)
        initial = await snapshot(dial)
        box = await dial.bounding_box()
        x, y = box['x'] + box['width']/2, box['y'] + box['height']/2
        gap = await dial.evaluate('''node => {
          const labels = [...node.querySelectorAll('.vals span')];
          const index = Number(node.getAttribute('aria-valuenow'));
          const center = label => label.offsetLeft + label.offsetWidth/2;
          return center(labels[index]) - center(labels[index-1]);
        }''')
        before_requests = await page.evaluate('requests.length')
        await page.mouse.move(x, y)
        await page.mouse.down()
        samples = []
        travel = round(gap+4)
        movements = [round(gap*.1), round(gap*.25), round(gap*.4), round(gap*.75), travel]
        for dx in movements:
            await page.mouse.move(x+dx, y)
            await paint(page)
            samples.append(await snapshot(dial))
            assert await page.evaluate('clicks === requests.length'), 'Expected one click per accepted detent'
        assert all(a['x'] < b['x'] for a,b in zip([initial]+samples, samples)), samples
        assert [s['selected'] for s in samples] == [initial_value]*3 + [preceding]*2, samples
        # The label strip must travel exactly as far as the captured thumb,
        # including when the selected value changes and label widths differ.
        for dx, sample in zip(movements, samples):
            assert abs(sample['x'] - initial['x'] - dx) < .1, (dx, initial, sample)
        assert await page.evaluate('(start) => requests.slice(start)', before_requests) == [dict(id=identifier,value=preceding)]
        timbres = await page.evaluate('(start) => clickTimbres.slice(start)', before_requests)
        assert len(set(timbres)) == 1, timbres
        await page.screenshot(path=str(output/f'{identifier}-held.png'))
        await page.mouse.up()
        await settled(dial)
        released = await snapshot(dial)
        assert abs(released['error']) < 1, released
        assert released['x'] < samples[-1]['x'], (released, samples[-1])
        # Reversal and cancellation restore the selected detent without extra input.
        await page.mouse.move(x, y)
        await page.mouse.down()
        await page.mouse.move(x-8, y)
        await paint(page)
        reverse = await snapshot(dial)
        assert reverse['x'] < released['x'], reverse
        await dial.dispatch_event('pointercancel', dict(pointerId=1))
        await page.mouse.up()
        await settled(dial)
        assert abs((await snapshot(dial))['error']) < 1
        assert await page.evaluate('requests.length') == before_requests+1
        # Dragging left selects the following label and follows the pointer too.
        await page.mouse.move(x, y)
        await page.mouse.down()
        await page.mouse.move(x-travel, y)
        await paint(page)
        assert (await snapshot(dial))['selected'] == initial_value
        assert abs((await snapshot(dial))['x'] - released['x'] + travel) < .1
        await page.mouse.up()
        await settled(dial)
        assert abs((await snapshot(dial))['error']) < 1
        # Keyboard remains discrete and retains its established direction.
        await dial.press('ArrowLeft')
        await settled(dial)
        assert (await snapshot(dial))['selected'] == preceding
        await dial.press('ArrowRight')
        await settled(dial)
        assert (await snapshot(dial))['selected'] == initial_value
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
        assert (await snapshot(dial))['selected'] == initial_value
        results[identifier] = dict(initial=initial, samples=samples, released=released, timbre=timbres[0])
    assert results['aperture']['timbre'] != results['shutter_speed']['timbre']
    # Relative tick strips retain their position across gestures and snap too.
    ticks = page.locator('.dial[data-id="control_ring"]')
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
    assert await page.evaluate('clicks === requests.length'), 'Snap or cancellation produced an extra click'
    assert await page.evaluate('new Set(clickTimbres).size === 3'), 'Dial timbres were not distinct'
    # Touch selects vibration when available; sub-detent movement and hard stops
    # must not produce an extra pulse. These mocks verify requests, not hardware.
    aperture = page.locator('.dial[data-id="aperture"]')
    async def touch_move(distance):
        await aperture.evaluate('''(node, distance) => node.addEventListener('pointerdown', event => {
          queueMicrotask(() => window.dispatchEvent(new PointerEvent('pointermove', {
            pointerId:event.pointerId,clientX:event.clientX+distance,clientY:event.clientY,buttons:1
          })));
        }, {once:true})''', distance)
        await aperture.tap()
        await settled(aperture)

    await page.evaluate('window.haptics = true')
    before = await page.evaluate('({requests:requests.length, clicks, vibrations:vibrations.length})')
    distance = await aperture.evaluate('''node => {
      const labels = [...node.querySelectorAll('.vals span')];
      const center = label => label.offsetLeft + label.offsetWidth/2;
      return center(labels[Number(node.getAttribute('aria-valuenow'))]) - center(labels.at(-1));
    }''')
    await touch_move(distance)
    after = await page.evaluate('(before) => ({steps:requests.length-before.requests, clicks:clicks-before.clicks, vibrations:vibrations.slice(before.vibrations)})', before)
    assert after == dict(steps=3, clicks=0, vibrations=[0,8,8,8]), after
    before = await page.evaluate('({requests:requests.length, clicks, vibrations:vibrations.length})')
    await touch_move(-40)
    after = await page.evaluate('(before) => ({steps:requests.length-before.requests, clicks:clicks-before.clicks, vibrations:vibrations.slice(before.vibrations)})', before)
    assert after == dict(steps=0, clicks=0, vibrations=[0]), after
    # Unsupported touch vibration falls back to the same quiet audio click.
    await page.evaluate('window.haptics = false')
    before = await page.evaluate('({requests:requests.length, clicks})')
    await touch_move(-distance)
    after = await page.evaluate('(before) => ({steps:requests.length-before.requests, clicks:clicks-before.clicks})', before)
    assert after == dict(steps=3, clicks=3), after
    await page.screenshot(path=str(output/'released.png'))
    assert not errors, errors
    (output/'result.json').write_text(json.dumps(dict(dials=results, requests=await page.evaluate('requests'),
        feedback=await page.evaluate('({clicks,vibrations,clickTimbres})')), indent=2)+'\n')
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
            print(f'{name}: pixel-for-pixel drag, release snap, reversal, cancellation, keyboard, touch and detent feedback passed')


if __name__ == '__main__':
    asyncio.run(main())
