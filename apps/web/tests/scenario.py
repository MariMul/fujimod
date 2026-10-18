"""Replay the real Movie-button fault twice through the development URL.

No response interception, injected errors, hardware access or UI substitutions.
Requires the configured local X70 DAT/runtime. Receipts go to an ignored directory.
"""

import argparse
import asyncio
import hashlib
import json
from pathlib import Path

from playwright.async_api import async_playwright


async def check(args: argparse.Namespace) -> None:
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch()
        try:
            receipts = []
            for run in range(2):
                context = await browser.new_context(
                    viewport=dict(width=1300, height=900)
                )
                await context.add_init_script("""
                  window.scenarioTrace = {sent:[], received:[], animation:[]};
                  // Observe the real renderer from fault entry through its settled frame.
                  new MutationObserver(() => {
                    const viewer = document.querySelector('#viewer');
                    if (viewer?.dataset.faultAnimation !== 'entering' || scenarioTrace.animation.length) return;
                    const sample = () => {
                      const canvas = document.querySelector('#lcd-source');
                      const c = canvas.getContext('2d');
                      scenarioTrace.animation.push({
                        phase: viewer.dataset.faultAnimation,
                        background: Array.from(c.getImageData(10,10,1,1).data),
                        alert: Array.from(c.getImageData(600,240,1,1).data),
                      });
                      if (viewer.dataset.faultAnimation === 'entering') requestAnimationFrame(sample);
                    };
                    sample();
                  }).observe(document, {subtree:true, attributes:true, attributeFilter:['data-fault-animation']});
                  const W = window.Worker;
                  window.Worker = class extends W {
                    constructor(...args) {
                      super(...args);
                      if (!String(args[0]).includes('pyodide-worker')) return;
                      this.addEventListener('message', ({data}) => {
                        if (['ready','error','cache'].includes(data.type))
                          scenarioTrace.received.push(JSON.parse(JSON.stringify(data)));
                      });
                      const send = this.postMessage.bind(this);
                      this.postMessage = (data, ...rest) => {
                        if (data.type === 'request') scenarioTrace.sent.push(data);
                        if (data.type === 'boot') scenarioTrace.config = data.config;
                        return send(data, ...rest);
                      };
                    }
                  };
                """)
                page = await context.new_page()
                errors = []
                page.on(
                    "pageerror", lambda error, errors=errors: errors.append(str(error))
                )
                await page.goto(args.url)
                await page.wait_for_function(
                    "document.querySelector('#live-label').textContent === 'STOPPED'",
                    timeout=240000,
                )
                await page.wait_for_function(
                    "document.querySelector('#viewer').dataset.faultAnimation === 'settled'"
                )
                trace = await page.evaluate("scenarioTrace")
                animation = trace["animation"]
                assert len(animation) >= 3 and animation[0]["phase"] == "entering", (
                    animation
                )
                before, after = animation[0], animation[-1]
                assert after["phase"] == "settled", animation
                assert before["alert"] != after["alert"], animation
                assert len({tuple(frame["background"]) for frame in animation}) >= 3, (
                    animation
                )
                for initial, final in zip(
                    before["background"][:3], after["background"][:3], strict=False
                ):
                    assert abs(final - initial * 0.68) <= 2, animation
                (args.output / f"animation-{run}.json").write_text(
                    json.dumps(animation, indent=2) + "\n"
                )
                faults = [
                    row["fault"] for row in trace["received"] if row["type"] == "error"
                ]
                assert len(faults) == 1, trace
                fault = faults[0]
                assert fault["kind"] == "unsupported-device", fault
                assert fault["details"]["device"] == "SensorCaptureDMA", fault
                assert "_latch" in str(fault["details"]), fault
                assert any(row["type"] == "ready" for row in trace["received"]), trace
                assert trace["config"]["execution"]["mode"] == "replay", trace["config"]
                assert all(
                    row.get("mode") != "resume"
                    for row in trace["received"]
                    if row["type"] == "cache"
                )
                inputs = [
                    row
                    for row in trace["sent"]
                    if row["action"] in ("press", "release")
                ]
                assert [(row["action"], row["options"]["id"]) for row in inputs] == [
                    ("press", "movie"),
                    ("release", "movie"),
                ], inputs
                assert await page.locator("#engine-status").is_visible()
                assert await page.locator("#restart").is_enabled()
                assert await page.locator("#error-report").is_enabled()
                assert await page.locator("#camera-title").inner_text() == "X70 · 1.10"
                assert await page.locator("#lcd-source").is_visible()
                pixels = await page.locator("#lcd-source").evaluate("c=>c.toDataURL()")
                await page.wait_for_timeout(500)
                assert (
                    await page.locator("#lcd-source").evaluate("c=>c.toDataURL()")
                    == pixels
                )
                assert not errors, errors
                # Ready state and actual fault evidence must repeat exactly across cold boots.
                receipt = dict(
                    lcd_sha256=hashlib.sha256(pixels.encode()).hexdigest(),
                    fault=fault,
                    ready=next(
                        row["state"]
                        for row in trace["received"]
                        if row["type"] == "ready"
                    ),
                    inputs=inputs,
                    final_advance=next(
                        row
                        for row in reversed(trace["sent"])
                        if row["action"] == "advance"
                    ),
                )
                receipts.append(receipt)
                (args.output / f"run-{run}.json").write_text(
                    json.dumps(receipt, indent=2) + "\n"
                )
                await page.screenshot(
                    path=str(args.output / f"run-{run}.png"), full_page=True
                )
                print(f"run {run}: real {fault['details']['device']} fault", flush=True)
                await context.close()
            assert receipts[0] == receipts[1], (
                "Cold replay evidence differs; inspect run receipts"
            )
            print(
                "PASS: both cold replays have identical ready state, inputs, LCD pixels and execution fault",
                flush=True,
            )
        finally:
            await browser.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True)
    parser.add_argument("--output", type=Path, required=True)
    asyncio.run(check(parser.parse_args()))
