"""Headed WebKit/iPhone speed, real reload and visible MENU latency gate.

Uses the normal page and observes its worker messages. No source replacement,
private runtime hooks or guest patches. Run capture.py for the photo/Playback
memory high-water mark; this gate measures steady shooting with Undo enabled.
"""

import argparse
import asyncio
import json
import time
from pathlib import Path

from browser_storage import clear_camera_storage
from capture import MENU_HEADER
from playwright.async_api import async_playwright


async def check(args: argparse.Namespace) -> None:
    args.output.mkdir(parents=True, exist_ok=False)
    result = dict(url=args.url, device="iPhone 13")
    async with async_playwright() as p:
        if args.persistent:
            options = dict(p.devices["iPhone 13"])
            options.pop("default_browser_type", None)
            context = await p.webkit.launch_persistent_context(
                str(args.output / "profile"), headless=False, **options
            )
            browser = context.browser
            assert browser is not None
            page = context.pages[0] if context.pages else await context.new_page()
        else:
            browser = await p.webkit.launch(headless=False)
            page = await browser.new_page(
                **p.devices["iPhone 13"], ignore_https_errors=True
            )
        result["browser"] = browser.version
        errors = []
        page.on("pageerror", lambda error, errors=errors: errors.append(str(error)))
        await page.add_init_script("""(() => {
          const Original = Worker;
          window.Worker = class extends Original {
            constructor(...args) {
              super(...args);
              if (!String(args[0]).includes('pyodide-worker')) return;
              window.__worker = this; window.__workerUrl=String(args[0]);
              this.addEventListener('message', ({data}) => {
                if (data.state) window.__state = data.state;
                if (data.type === 'ready') window.__ready = data;
                if (data.type === 'cache') window.__cache = data;
              });
            }
          };
        })();""")
        if args.cold:
            await clear_camera_storage(page, args.url)
        try:
            result["loads"] = []
            for phase in ("initial", "reload"):
                started = time.monotonic()
                if phase == "initial":
                    await page.goto(args.url)
                else:
                    await page.reload()
                if args.dat:
                    await page.locator("#firmware-file").set_input_files(str(args.dat))
                await page.wait_for_function(
                    'document.querySelector("#live-label").textContent === "LIVE" || document.querySelector("#status").classList.contains("error")',
                    timeout=180000,
                )
                assert await page.locator("#live-label").inner_text() == "LIVE", (
                    await page.locator("#fault-raw").inner_text()
                )
                elapsed = time.monotonic() - started
                await page.evaluate(
                    'window.__poll=setInterval(()=>__worker.postMessage({type:"recovery_status"}),500)'
                )
                await page.wait_for_function(
                    "__cache?.saved || __cache?.cacheFailure", timeout=90000
                )
                cache = await page.evaluate("__cache")
                assert cache["saved"], cache
                assert cache["mode"] in ("cold", "resume") and (
                    phase == "initial" or cache["mode"] == "resume"
                ), cache
                result["loads"].append(
                    dict(
                        phase=phase,
                        seconds=elapsed,
                        cache=cache,
                        worker=await page.evaluate("__workerUrl"),
                        runtime=await page.evaluate("__ready.runtime"),
                        marks=await page.evaluate(
                            'performance.getEntriesByType("mark").map(e=>({name:e.name,ms:e.startTime}))'
                        ),
                    )
                )
                print(f"{phase} LIVE {elapsed:.3f}s ({cache['backend']})", flush=True)
                await page.screenshot(path=str(args.output / f"{phase}.png"))
            await page.wait_for_function(
                "window.cameraPerformance?.runtime?.presentedImageFrames > 0",
                timeout=30000,
            )
            sample = "({tick:__state.virtual_ticks,wall:performance.now(),image:__state.image_frames,presented:cameraPerformance.runtime.presentedImageFrames})"
            before = await page.evaluate(sample)
            await page.wait_for_timeout(args.seconds * 1000)
            after = await page.evaluate(sample)
            guest = (after["tick"] - before["tick"]) / 12000000
            result["preview"] = dict(
                seconds=args.seconds,
                speed=guest * 1000 / (after["wall"] - before["wall"]),
                image_guest_fps=(after["image"] - before["image"]) / guest,
                presented_guest_fps=(after["presented"] - before["presented"]) / guest,
            )
            menu = page.locator('button[data-id="menu_ok"]')
            await menu.scroll_into_view_if_needed()
            await page.wait_for_timeout(250)
            started = time.monotonic()
            await menu.tap()
            header = """async()=>{
              const rgba=document.querySelector('#lcd-source').getContext('2d').getImageData(0,0,720,48).data;
              const rgb=new Uint8Array(720*48*3);
              for(let s=0,t=0;s<rgba.length;s+=4,t+=3)rgb.set(rgba.subarray(s,s+3),t);
              const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',rgb)),b=>b.toString(16).padStart(2,'0')).join('');
              return hash;
            }"""
            deadline = time.monotonic() + 60
            while await page.evaluate(header) != MENU_HEADER:
                if time.monotonic() > deadline:
                    raise AssertionError("Exact English MENU pixels did not arrive")
                await page.wait_for_timeout(25)
            result["menu_seconds"] = time.monotonic() - started
            await page.locator("#viewer").scroll_into_view_if_needed()
            await page.screenshot(path=str(args.output / "menu.png"))
            result["runtime"] = await page.evaluate("cameraPerformance.runtime")
            assert result["loads"][1]["seconds"] < args.max_warm_seconds, result[
                "loads"
            ][1]
            assert result["preview"]["speed"] >= args.min_speed, result["preview"]
            assert result["preview"]["image_guest_fps"] >= 9.5, result["preview"]
            assert result["preview"]["presented_guest_fps"] >= 9.5, result["preview"]
            assert not errors, errors
        finally:
            result["page_errors"] = errors
            (args.output / "result.json").write_text(
                json.dumps(result, indent=2) + "\n"
            )
            await browser.close()
    print(
        json.dumps(
            dict(
                warm=result["loads"][1]["seconds"],
                preview=result["preview"],
                menu_seconds=result["menu_seconds"],
            )
        ),
        flush=True,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://127.0.0.1:5190/")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dat", type=Path)
    parser.add_argument(
        "--cold",
        action="store_true",
        help="Clear this app's test-origin asset/checkpoint stores before loading",
    )
    parser.add_argument(
        "--persistent",
        action="store_true",
        help="Use a fresh on-disk profile, including OPFS, instead of private browsing",
    )
    parser.add_argument("--seconds", type=int, default=30)
    parser.add_argument("--min-speed", type=float, default=0.5)
    parser.add_argument("--max-warm-seconds", type=float, default=5)
    asyncio.run(check(parser.parse_args()))
