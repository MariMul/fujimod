"""Verify development SD boot through the served app and passive RAM inspection.

Supply the image manifest and mod build directory used by the configured patch
release. The debugger pauses the worker and reads RAM; no guest state is edited.
"""
import argparse
import asyncio
import hashlib
import json
from pathlib import Path

from playwright.async_api import async_playwright


async def memory_hashes(browser, page, ranges):
    cdp = await browser.new_browser_cdp_session()
    targets = (await cdp.send('Target.getTargets'))['targetInfos']
    target = next(t for t in targets if t['type'] == 'worker' and 'pyodide-worker' in t['url'])
    session = (await cdp.send('Target.attachToTarget', dict(targetId=target['targetId'], flatten=False)))['sessionId']
    pending, sequence = {}, 0
    paused = asyncio.get_running_loop().create_future()
    def receive(event):
        if event['sessionId'] != session:
            return
        message = json.loads(event['message'])
        if 'id' in message and message['id'] in pending:
            pending.pop(message['id']).set_result(message)
        elif message.get('method') == 'Debugger.paused' and not paused.done():
            paused.set_result(message['params'])
    cdp.on('Target.receivedMessageFromTarget', receive)
    async def send(method, params=None):
        nonlocal sequence
        sequence += 1
        future = asyncio.get_running_loop().create_future()
        pending[sequence] = future
        await cdp.send('Target.sendMessageToTarget', dict(sessionId=session,
            message=json.dumps(dict(id=sequence, method=method, params=params or {}))))
        reply = await asyncio.wait_for(future, 30)
        assert 'error' not in reply, reply
        return reply['result']
    try:
        await send('Debugger.enable')
        source_text = await (await page.request.get(target['url'])).text()
        line = next(i for i, value in enumerate(source_text.splitlines()) if 'if (data.type === "recovery_status")' in value) + 1
        breakpoint = await send('Debugger.setBreakpointByUrl', dict(url=target['url'], lineNumber=line))
        await page.evaluate("modBoot.worker.postMessage({type:'recovery_status'})")
        frames = (await asyncio.wait_for(paused, 30))['callFrames']
        source = ('import hashlib,json\njson.dumps([hashlib.sha256(session.board.read_memory(a,n)).hexdigest() '
                  f'for a,n in {ranges!r}])')
        failures = []
        for frame in frames:
            result = await send('Debugger.evaluateOnCallFrame', dict(callFrameId=frame['callFrameId'],
                expression='runtime().runPython(' + json.dumps(source) + ')', returnByValue=True))
            if 'exceptionDetails' not in result and result.get('result', {}).get('type') == 'string':
                return json.loads(result['result']['value'])
            failures.append(result.get('exceptionDetails', {}).get('text', 'No string result'))
        raise AssertionError(f'No runtime scope in paused worker: {failures}')
    finally:
        await send('Debugger.removeBreakpoint', dict(breakpointId=breakpoint['breakpointId']))
        await send('Debugger.resume')
        await cdp.send('Target.detachFromTarget', dict(sessionId=session))
        await cdp.detach()


async def check(args):
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = json.loads(args.image_manifest.read_text())
    ranges, expected = [], []
    for word in manifest['mods']['words']:
        ranges.append((int(word['address'], 16), 4))
        expected.append(hashlib.sha256(int(word['new'], 16).to_bytes(4, 'little')).hexdigest())
    word_count = len(ranges)
    for region in manifest['mods']['regions']:
        image = (args.mods / region['file']).read_bytes()
        for section in region['sections']:
            if section['name'] in ('.data', '.bss', '.x70mod.shared') or not section['size']:
                continue
            offset, size = section['offset'], section['size']
            ranges.append((int(region['address'], 16) + offset, size))
            expected.append(hashlib.sha256(image[offset:offset+size]).hexdigest())
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch()
        try:
            context = await browser.new_context(viewport=dict(width=1300, height=900))
            await context.add_init_script('''
              window.modBoot = {messages:[]};
              const W = window.Worker;
              window.Worker = class extends W {
                constructor(...args) {
                  super(...args);
                  if (!String(args[0]).includes('pyodide-worker')) return;
                  modBoot.worker = this;
                  this.addEventListener('message', ({data}) => {
                    if (['ready','error','cache'].includes(data.type)) modBoot.messages.push(data);
                  });
                  const send = this.postMessage.bind(this);
                  this.postMessage = (data, ...rest) => {
                    if (data.type === 'boot') modBoot.config = data.config;
                    return send(data, ...rest);
                  };
                }
              };
            ''')
            page = await context.new_page()
            await page.goto(args.url)
            await page.wait_for_function("modBoot.messages.some(m => ['ready','error'].includes(m.type))", timeout=240000)
            receipt = await page.evaluate('({config:modBoot.config,messages:modBoot.messages})')
            (args.output / 'startup.json').write_text(json.dumps(receipt, indent=2)+'\n')
            assert receipt['config'] and receipt['config']['bootSource'] == 'sd', receipt
            assert not [m for m in receipt['messages'] if m['type'] == 'error'], receipt
            assert any(m['type'] == 'ready' for m in receipt['messages']), receipt
            ready = next(m for m in receipt['messages'] if m['type'] == 'ready')
            assert ready['state']['instructions'] < 70_000_000, ready['state']['instructions']
            assert ready['state']['virtual_ticks'] < 24_000_000, ready['state']['virtual_ticks']
            await page.wait_for_function('window.cameraPerformance !== undefined')
            pace = await page.evaluate('window.cameraPerformance')
            # Boot execution must remain in the displayed speed measurement.
            assert pace['virtualSeconds'] >= ready['state']['virtual_ticks'] / 12_000_000, pace
            boot_wall = next(s['seconds'] for s in ready['runtime']['bootStages'] if s['name'] == 'ready-reply')
            assert pace['wallSeconds'] >= boot_wall, (pace, boot_wall)
            receipt['pace'] = pace
            actual = await memory_hashes(browser, page, ranges)
            for address, found, wanted in zip(ranges, actual, expected, strict=True):
                assert found == wanted, (address, found, wanted)
            receipt.update(verified_mod_words=word_count, verified_immutable_sections=len(ranges)-word_count,
                           sd_image_sha256=manifest['ffboot']['sha256'])
            await page.locator('#view-lcd').click()
            await page.locator('#viewer').screenshot(path=str(args.output / 'modded-camera.png'))
            (args.output / 'result.json').write_text(json.dumps(receipt, indent=2)+'\n')
            print(f"PASS: {word_count} mod words and {len(ranges)-word_count} immutable sections in running browser RAM")
        finally:
            await browser.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--image-manifest', type=Path, required=True)
    parser.add_argument('--mods', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    asyncio.run(check(parser.parse_args()))
