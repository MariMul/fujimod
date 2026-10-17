# Python walkthrough recording

Install FujiMod with its `walkthroughs` extra and put `ffmpeg` on PATH.
The library has no camera, firmware or repository dependency.

```python
from fujimod.walkthroughs import walkthrough

@walkthrough('Use a feature')
def example(camera, guide):
    with guide.step('Choose the option', 'Turn the dial.', hold=2):
        camera.press('right')
        assert camera.current_option() == 'example'
```

`discover(path)` imports trusted local Python and returns decorated functions
from that file. Several functions may share a file. The host derives identity
and mod ownership from paths; authors provide only titles, prose and code.

Construct a fresh adapter per function, then call
`Guide(camera, fresh_output, identity=identity, title=title).run(function)`.
The host owns identity: include build and execution/source hashes suitable for
its coverage gate. Python assertions must be enabled.

A camera adapter implements:

- `start_recording(guide)`: attach recording to existing input/execution paths.
- `capture()`: settle and return clean PNG bytes for the current screen.
- `stop_recording()`: remove recording hooks, including on failure.

It calls `guide.frame(png, seconds)` at monotonically increasing guest times,
`guide.input(name, pressed, seconds)` for real input edges (returns an index),
and `guide.interaction(surface, item, input_index=..., **evidence)` for observed
interactions. Frame sampling must not drive device state. Input badges are
rendered only into video, in the bottom-right, fading after release.
The adapter should sample immediately before an input edge so its overlay
starts and stops at that edge. It supplies evidence rather than accepting
coverage declarations from walkthrough authors.

`guide.step(title, text='', hold=2)` captures on successful block exit, then
holds that observation for presentation seconds without advancing guest time.
An assertion failure prevents capture and makes the whole report fail.
`guide.fixture(description)` records setup interventions and suppresses their
coverage. Fixtures are independent of guide steps.

`guide.json` schema 1 contains identity, title, pass/fail, numbered steps,
clean screenshot paths/hashes, video segment timestamps, input edges,
interactions, fixtures, and artifact hashes. Failed runs keep diagnostics and
never become successful guides. Video is sampled at 15 fps, independent of
host execution speed. Hosts can build websites from these outputs without
running the walkthrough source during the website build itself.
