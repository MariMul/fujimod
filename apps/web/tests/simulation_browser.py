"""Bevy migration gate against the real web app and firmware emulator.

Run against the managed preview with its local DAT configured, or supply --dat.
Outputs go to a fresh ignored directory. No camera/USB access or guest fixtures.
"""
import argparse
import asyncio
import json
from pathlib import Path
from playwright.async_api import async_playwright


async def run(args):
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as playwright:
        browser = await getattr(playwright, args.browser).launch()
        page = await browser.new_page(viewport=dict(width=1440, height=1000))
        errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        await page.goto(args.url)
        if args.dat:
            await page.locator('#firmware-file').set_input_files(str(args.dat))
        await page.wait_for_function("document.querySelector('#viewer').dataset.renderer==='bevy'")
        await page.wait_for_function("document.querySelector('#live-label').textContent==='LIVE'", timeout=180000)
        await page.wait_for_function("Number(document.querySelector('#viewer').dataset.frames)>3", timeout=60000)
        await page.wait_for_function("JSON.parse(document.querySelector('#viewer').dataset.simulation).ready")
        async def state():
            return json.loads(await page.locator('#viewer').get_attribute('data-simulation'))
        if 'environment=' in args.url:
            await page.wait_for_function("JSON.parse(document.querySelector('#viewer').dataset.simulation).sensorSequence>0",timeout=60000)
            assert not await page.locator('#viewer').get_attribute('data-scene-error')
        initial = await state()
        assert initial['version'] == '0.20.0'
        canvas = page.locator('#simulation-canvas')
        box = await canvas.bounding_box()
        x, y = box['x'] + box['width'] * .5, box['y'] + box['height'] * .12
        await page.mouse.move(x, y)
        await page.mouse.down()
        await page.mouse.move(x+90, y+30, steps=12)
        await page.mouse.up()
        await page.wait_for_timeout(150)
        orbited = await state()
        assert initial['cameraPosition'] == orbited['cameraPosition']
        assert initial['cameraRotation'] == orbited['cameraRotation']
        assert initial['observerPosition'] != orbited['observerPosition']
        await canvas.click(button='right')
        await page.wait_for_function("document.querySelector('#viewer').dataset.mode==='walk'")
        before = await state()
        await page.keyboard.down('w')
        await page.wait_for_timeout(1000)
        await page.keyboard.up('w')
        walked = await state()
        distance = sum((a-b)**2 for a,b in zip(before['cameraPosition'], walked['cameraPosition']))**.5
        assert .8 < distance < 1.9, distance
        await canvas.click(button='right')
        await page.wait_for_function("document.querySelector('#viewer').dataset.mode==='inspect'")
        await page.wait_for_timeout(300)
        fixed = await state()
        await page.keyboard.press('w')
        await page.wait_for_timeout(100)
        assert (await state())['cameraPosition'] == fixed['cameraPosition']
        # Tap a visible rear button using the projection produced by Bevy itself.
        await page.locator('#face-screen').click()
        await page.wait_for_timeout(150)
        targets=(await state())['targets']
        identifier = next(i for i in ['disp','q','menu_ok'] if targets.get(i,{}).get('visible'))
        point=targets[identifier]
        box=await canvas.bounding_box()
        await page.mouse.click(box['x']+point['x'],box['y']+point['y'])
        await page.wait_for_function("id=>document.querySelector('.model-input-feedback').dataset.input===id",arg=identifier)
        await page.screenshot(path=str(args.output/'bevy.png'))
        assert not errors, errors
        report=dict(renderer=initial['renderer'],version=initial['version'],walk_distance=distance,
            inspect_preserves_camera=True,button=identifier,environment=initial['environmentReady'],errors=errors)
        (args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))
        await browser.close()


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url',required=True)
    parser.add_argument('--dat',type=Path)
    parser.add_argument('--browser',choices=['chromium','webkit'],default='chromium')
    parser.add_argument('--output',type=Path,required=True)
    asyncio.run(run(parser.parse_args()))
