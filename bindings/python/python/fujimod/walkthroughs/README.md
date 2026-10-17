# Python walkthrough recording

Install FujiMod with its `walkthroughs` extra and put `ffmpeg` on PATH.
The library has no camera, firmware or repository dependency.

```python
from fujimod.walkthroughs import open_menu, walkthrough

settings = open_menu('settings_item', title='Open Settings')

@walkthrough('Use a feature', start_from=settings)
def example(camera, guide):
    with guide.step('Choose the option', 'Turn the dial.'):
        camera.press('right')
        assert camera.current_option() == 'example'
```

A normal walkthrough can also be a `start_from` prerequisite. Branches start
independently from their parent's completed state. A host must reject cycles
and unresolved prerequisites before execution. The current runner replays
prerequisites in a fresh camera, runs their assertions and settling, then
starts the child's recording. Replay contributes no child steps, input records
or coverage. Failed prerequisites prevent the child from executing. Checkpoint
caching is not part of the authoring API.

`open_menu(item, title=..., start_from=None)` creates an ordinary walkthrough
with two illustrated steps: select the item, then open it. Its adapter implements
`menu_label(item)`, `select_menu_item(item)` and `open_menu_item(item)`. Factory
identity comes from the module variable name; decorated functions use their
function name. Filenames and containing directories provide the remaining
identity and ownership. There are no explicit IDs or mod lists.

`discover(path)` imports one file or all public Python files in a directory in
one fresh package. Several walkthroughs may share a file; relative imports can
reference prerequisites in other files. Discovery reloads local source instead
of accepting stale bytecode, and imported guides keep their original ownership.

Construct a fresh camera adapter per segment, then call
`Guide(camera, fresh_output, identity=identity, title=title).run(function,
prerequisites=[(parent_identity, parent_function), ...])` with prerequisites in
root-to-parent order. The host owns build and execution/source identity. Python
assertions must be enabled.

A camera adapter implements:

- `start_recording(guide)`: attach observation to existing execution/input paths.
- `begin_segment()`: reset the local guest clock and supply the first frame after replay.
- `capture()`: settle and return clean PNG bytes for the current screen.
- `stop_recording()`: remove recording hooks, including on failure.

It calls `guide.frame(png, seconds)` at monotonically increasing guest times,
`guide.input(name, pressed, seconds)` for real input edges (returns an index),
and `guide.interaction(surface, item, input_index=..., **evidence)` for observed
interactions. Frame sampling never drives device state. Only non-fixture press
inputs can anchor coverage. Input badges appear in the video's bottom-right,
fading for 0.4 presentation seconds after release. Input edges flush the prior
interval before changing the badge.

`guide.step(title, text='', hold=.75)` captures on successful block exit, then
holds that observation without advancing guest time. `guide.pause(seconds=.35)`
shows a settled action result without creating a documentation step. Explicit
holds are never compressed. Unchanged frames without active/fading inputs keep
at most 0.4 seconds of guest waiting; transitions and input intervals remain.
Input records retain their actual guest times and presentation timestamps.
Neither video pacing nor holds change emulator execution or CPU speed.

A zero hold adds no presentation time. The final frame is retained. Failed steps
and fixtures fail the run even if author code catches their exception.
`guide.fixture(description)` records setup interventions and suppresses coverage.

`guide.json` schema 2 contains identity, title, prerequisite results, pass/fail,
numbered steps, screenshot hashes, local video timestamps, input edges,
interactions, fixtures, presentation timing and artifact hashes. `poster.png`
is the segment's actual starting frame, captured after prerequisite replay.
The host records `start_from` as the parent identity so consumers can link guides.
Failed runs retain diagnostics and never qualify coverage. Video is sampled at
15 fps; websites consume these artifacts without executing walkthrough source.
