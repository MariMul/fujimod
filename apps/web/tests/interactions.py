"""Shared visible-control gestures for the data-driven camera panel."""
import asyncio


async def tap(locator):
    await locator.scroll_into_view_if_needed()
    await asyncio.sleep(.2)
    await locator.tap()


async def set_dial(page, identifier, value):
    dial=page.locator(f'.dial[data-id="{identifier}"]')
    values=await dial.locator('.vals span').evaluate_all('(items)=>items.map(item=>item.dataset.value)')
    target=values.index(value)
    current=int(await dial.get_attribute('aria-valuenow'))
    await dial.focus()
    for _ in range(abs(target-current)):
        await dial.press('ArrowRight' if target>current else 'ArrowLeft')
    assert await dial.get_attribute('aria-valuetext')==value


async def segment(page, identifier, value):
    await tap(page.locator(f'.control-node[data-id="{identifier}"] button[data-value="{value}"]'))


async def touch_dial(page, identifier, distance=32):
    """Trusted WebKit touch start; Playwright lacks a native touch-move API."""
    dial=page.locator(f'.dial[data-id="{identifier}"]')
    await dial.scroll_into_view_if_needed()
    await asyncio.sleep(.2)
    await dial.evaluate('''(node,dx)=>node.addEventListener('pointerdown',event=>{
      window.__dialTouch={trusted:event.isTrusted,type:event.pointerType};
      queueMicrotask(()=>{for(let i=1;i<=8;i++)window.dispatchEvent(new PointerEvent('pointermove',{
        pointerId:event.pointerId,pointerType:event.pointerType,isPrimary:true,clientX:event.clientX+dx*i/8,
        clientY:event.clientY,buttons:1,bubbles:true}));});
    },{once:true})''',distance)
    await dial.tap()
    assert await page.evaluate('window.__dialTouch')==dict(trusted=True,type='touch')
