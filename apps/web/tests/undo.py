"""Check Undo visibility against real camera history through the browser worker.

Run against a Vite development server with local DAT/runtime inputs. Optional
--sources overrides the Python archive. Receipts go to a fresh output directory.
No camera or USB access.
"""

import argparse
import asyncio
import json
from pathlib import Path

from playwright.async_api import Playwright, async_playwright


async def check(playwright: Playwright, args: argparse.Namespace, name: str) -> None:
    out = args.output / name
    out.mkdir()
    browser = await getattr(playwright, name).launch(headless=False)
    context = await browser.new_context(**playwright.devices["iPhone 13"])
    page = await context.new_page()
    errors = []
    page.on("pageerror", lambda error, errors=errors: errors.append(str(error)))

    if args.sources:
        await page.route(
            "**/sources.zip",
            lambda route: route.fulfill(
                path=str(args.sources.resolve()), content_type="application/zip"
            ),
        )
    await context.add_init_script("""
      window.undoErrors = []; window.failOnReady = true;
      window.stopCamera = () => window.cameraWorker.postMessage({
        type:'request', id:-100, action:'advance', options:{limit:200001,pause_at_tick:0}
      });
      const WorkerClass = window.Worker;
      window.Worker = class extends WorkerClass {
        constructor(...args) {
          super(...args);
          if (!String(args[0]).includes('pyodide-worker')) return;
          window.cameraWorker = this;
          this.addEventListener('message', ({data}) => {
            if (data.type === 'cache') window.cameraCache = data;
            if (data.type === 'error') window.undoErrors.push(data);
            if (data.type === 'ready' && window.failOnReady) window.stopCamera();
          });
        }
      };
    """)

    async def stopped(available: bool) -> None:
        await page.wait_for_function(
            "document.querySelector('#live-label').textContent === 'STOPPED'",
            timeout=180000,
        )
        assert await page.locator("#undo").is_visible() == available
        assert await page.locator("#undo").is_disabled() != available
        assert await page.locator("#restart").is_visible()
        assert await page.locator("#error-report").is_visible()
        assert (
            "not enough camera history"
            not in await page.locator("#fault-raw").inner_text()
        )

    try:
        await page.goto(args.url)
        await page.wait_for_function(
            "document.querySelector('#live-label').textContent === 'STOPPED'",
            timeout=180000,
        )
        await stopped(
            await page.evaluate(
                "Boolean(window.cameraCache?.history?.snapshots.length)"
            )
        )
        await page.screenshot(path=str(out / "early-fault.png"))
        await page.evaluate("window.failOnReady = false")
        await page.locator("#restart").click()
        await page.wait_for_function(
            "document.querySelector('#live-label').textContent === 'LIVE'",
            timeout=180000,
        )
        await page.evaluate(
            'window.cacheTimer = setInterval(() => window.cameraWorker.postMessage({type:"recovery_status"}), 250)'
        )
        await page.wait_for_function(
            """(() => {
          const h = window.cameraCache?.history;
          return h?.snapshots.length && h.ticks - h.snapshots[0] >= 120000000;
        })()""",
            timeout=180000,
        )
        await page.evaluate("clearInterval(window.cacheTimer); window.stopCamera()")
        await stopped(True)
        await page.screenshot(path=str(out / "available-history.png"))
        for _ in range(3):
            await page.locator("#undo").click()
            await page.wait_for_function(
                "document.querySelector('#live-label').textContent === 'LIVE'",
                timeout=30000,
            )
            await page.evaluate("window.stopCamera()")
            await stopped(True)
        await page.screenshot(path=str(out / "reusable-history.png"))
        await page.locator("#undo").click()
        await page.wait_for_function(
            "document.querySelector('#live-label').textContent === 'LIVE'",
            timeout=30000,
        )
        assert errors == [], errors
        result = dict(
            browser=name,
            faults=await page.evaluate("window.undoErrors"),
            page_errors=errors,
        )
        (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(
            f"{name}: repeated faults retain usable Undo; every Undo resumes",
            flush=True,
        )
    except BaseException:
        await page.screenshot(path=str(out / "failure.png"))
        (out / "failure.json").write_text(
            json.dumps(
                await page.evaluate(
                    '({errors:window.undoErrors,cache:window.cameraCache,status:document.querySelector("#status")?.textContent})'
                ),
                indent=2,
            )
        )
        raise
    finally:
        await browser.close()


async def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://127.0.0.1:5191/")
    parser.add_argument("--sources", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--engines", nargs="+", default=["chromium", "webkit"])
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    async with async_playwright() as playwright:
        for name in args.engines:
            await check(playwright, args, name)


if __name__ == "__main__":
    asyncio.run(main())
