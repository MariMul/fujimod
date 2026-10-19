"""Check real loading presentation and FujiMod/internal boot switching on the preview."""
import argparse
import asyncio
import json
from pathlib import Path

from playwright.async_api import async_playwright


async def run(args):
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(headless=False, args=['--enable-unsafe-webgpu'])
        context = await browser.new_context(viewport={'width': 1440, 'height': 1000})
        await context.add_init_script('''
          window.presentationBoots = [];
          const OriginalWorker = window.Worker;
          window.Worker = class extends OriginalWorker {
            constructor(...args) {
              super(...args);
              const send = this.postMessage.bind(this);
              this.postMessage = (data, ...rest) => {
                if (data.type === 'boot') presentationBoots.push({
                  source: data.config.bootSource, identities: data.seedIdentity, inputs: data.config.inputs,
                });
                return send(data, ...rest);
              };
            }
          };
        ''')
        page = await context.new_page()
        errors = []
        page.on('pageerror', lambda error: (errors.append(str(error)), print(str(error), flush=True)))
        page.on('console', lambda message: print(message.text, flush=True) if message.type == 'error' else None)
        await page.goto(args.url)
        await page.locator('#workspace').scroll_into_view_if_needed()
        await page.screenshot(timeout=120000, path=str(args.output / 'startup.png'))
        await page.wait_for_function("document.querySelector('#viewer').dataset.renderer === 'bevy' || document.querySelector('#viewer').dataset.rendererError", timeout=120000)
        renderer_error = await page.locator('#viewer').get_attribute('data-renderer-error')
        assert not renderer_error, renderer_error
        # Capture real boot progress if model initialization finishes before boot.
        await page.wait_for_function("JSON.parse(document.querySelector('#viewer').dataset.simulation || '{}')?.ready", timeout=120000)
        await page.screenshot(timeout=120000, path=str(args.output / 'initial.png'))
        async def ready(source, count):
            await page.wait_for_function("([source,count]) => presentationBoots.length >= count && presentationBoots.at(-1).source === source && document.querySelector('#live-label').textContent === 'LIVE'", arg=[source, count], timeout=240000)
            await page.wait_for_function("!document.querySelector('#firmware-mode button').disabled")
            await page.locator('#controls button[data-id="menu_ok"]').wait_for()
            assert await page.locator('#controls button').count() > 10
            print(f'{source} boot {count} ready with Solid controls', flush=True)
        await ready('sd', 1)
        modes = page.get_by_role('group', name='Firmware mode')
        modded = modes.get_by_role('button', name='FujiMod', exact=True)
        unmodded = modes.get_by_role('button', name='Unmodded', exact=True)
        await page.locator('#viewer-settings-toggle').click()
        assert await modded.get_attribute('aria-pressed') == 'true'
        await page.screenshot(timeout=120000, path=str(args.output / 'fujimod.png'))
        await page.locator('#face-lens').click()
        await page.wait_for_timeout(350)
        await page.screenshot(timeout=120000, path=str(args.output / 'front.png'))
        await page.locator('#face-screen').click()
        await page.locator('#viewer-settings-toggle').click()
        await unmodded.click()
        await ready('internal', 2)
        assert await unmodded.get_attribute('aria-pressed') == 'true'
        await page.screenshot(timeout=120000, path=str(args.output / 'unmodded.png'))
        await page.locator('#viewer-settings-toggle').click()
        await modded.click()
        await ready('sd', 3)
        await page.locator('#view-lcd').click()
        await page.wait_for_function("document.querySelector('#viewer').classList.contains('lcd-mode') && !document.querySelector('#lcd-source').hidden")
        assert await page.locator('#view-lcd').get_attribute('aria-pressed') == 'true'
        assert await page.locator('#lcd-source').evaluate('node => node.getContext("2d").getImageData(0,0,node.width,node.height).data.some(value => value > 0)')
        await page.screenshot(path=str(args.output / 'lcd.png'))
        await page.locator('#view-3d').click()
        assert await page.locator('#view-3d').get_attribute('aria-pressed') == 'true'
        inputs_url = await page.evaluate('new URL(presentationBoots.at(-1).inputs,document.baseURI).href')
        input_table = (await (await page.request.get(inputs_url)).json())['inputs']
        unavailable = {}
        # Apply every selectable value. An unmodeled input must match the
        # runtime's explicit refusal and remain disabled for every value.
        for identifier in ['auto','focus_mode','lcd_tilt']:
            group = page.locator(f'#controls .control-node[data-id="{identifier}"] .seg')
            buttons = group.locator('button')
            values = await buttons.evaluate_all('nodes => nodes.map(node => node.dataset.value)')
            assert len(values) >= 2, identifier
            declared = next(entry for entry in input_table if entry['id'] == identifier)
            if declared.get('binding') is None or declared.get('modeled') is False:
                for value in values:
                    assert await group.locator(f'button[data-value="{value}"]').is_disabled(), (identifier,value)
                unavailable[identifier] = declared.get('unmodelled_reason', 'No modeled hardware binding')
                continue
            for value in [values[-1], *values]:
                option = group.locator(f'button[data-value="{value}"]')
                assert await option.is_enabled(), (identifier,value)
                sequence = await page.locator('#viewer').get_attribute('data-frames')
                await option.click()
                await page.wait_for_function('([id,value]) => document.querySelector(`#controls .control-node[data-id="${id}"] button[data-value="${value}"]`)?.getAttribute("aria-pressed") === "true"', arg=[identifier,value])
                await page.wait_for_function('sequence => Number(document.querySelector("#viewer").dataset.frames) > Number(sequence) + 1', arg=sequence, timeout=30000)
                assert await page.locator('#live-label').inner_text() == 'LIVE'
                await page.screenshot(path=str(args.output / f'{identifier}-{value}.png'))
        await page.locator('#controls .control-node[data-id="lcd_tilt"] button[data-value="normal"]').click()
        # Card insertion state and listing come from the running guest, not UI fixtures.
        await page.locator('#card-files .r').first.wait_for(timeout=60000)
        for present in ['false','true']:
            await page.locator(f'#card-section button[data-card-present="{present}"]').click()
            await page.wait_for_function('value => document.querySelector(`#card-section button[data-card-present="${value}"]`)?.getAttribute("aria-pressed") === "true"', arg=present)
            await page.screenshot(path=str(args.output / f'card-{present}.png'))
        await page.locator('#card-files .r').first.wait_for(timeout=60000)
        await page.set_viewport_size({'width': 390, 'height': 844})
        await page.locator('#workspace').scroll_into_view_if_needed()
        await page.screenshot(timeout=120000, path=str(args.output / 'phone.png'))
        assert not errors, errors
        report = {'url': args.url, 'boots': await page.evaluate('presentationBoots'), 'errors': errors, 'unavailable_inputs': unavailable}
        (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report))
        await browser.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--output', required=True, type=Path)
    asyncio.run(run(parser.parse_args()))
