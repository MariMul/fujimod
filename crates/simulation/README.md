# Camera environment simulation

Shared Rust/Bevy 0.20.0 world, physical camera rig and observer rendering.
The web app uses this renderer for its camera model, picking, dial pivots and
firmware LCD texture. Environments live beside the implementation in
`environments/`; native and browser rendering consume the same prepared asset.
There are no device-specific quality variants.

## Interaction and time

The player is 1.75 m tall and holds the physical camera at 1.45 m, 0.30 m forward.
In Walk mode, WASD moves at 1.4 m/s and captured mouse movement turns the player
and camera. Right click enters Inspect, releases the mouse and moves the
observer close to the camera's rear glass. Dragging orbits the observer; the
physical camera pose and sensor image stay fixed. Right click returns to Walk.
The on-screen Walk/Inspect toggle and touch stick/drag issue the same commands.
Focus loss clears movement. The controller is in Rust; browser event capture
and DOM controls stay in the web app.

Interactive movement uses real elapsed time, independently of guest execution
speed. `Simulation::advance(seconds)` provides explicit time for scenarios.
Native rendering can use Bevy's `TimeUpdateStrategy::ManualDuration`, as in
`examples/render_scenario.rs`. Observer animation never advances firmware.
`Bridge::snapshot` and `Bridge::take_sensor` expose output and never advance the
world. GPU readback completion is asynchronous; a frame retains its render
request's world time, sequence and physical camera pose.

The observer sees world layer 0 and camera-body layer 1. The sensor sees only
layer 0. It renders a 768×512 image using physical focal length/sensor size and
Bevy's depth-of-field effect. Focus/aperture are explicit scenario inputs through
`Command::Optics`. Physical body width and initial optics come from the selected
body definition. The generic headless default is a declared 50 mm / 24 mm sensor-height
profile; it is not a camera calibration. These are declared approximations, not recovered
actuator calibration. No firmware PC, private RAM or software mode determines
world behavior. No autofocus decision is supplied by the environment.

The web firmware engine currently runs in the existing Pyodide worker. Its
RGB image-input adapter remains the boundary to that runtime, including pixel
transfer. The world and renderer share one Bevy world; this does **not** yet
merge the Pyodide execution backend into the Bevy Wasm module. Native callers
can consume `SensorFrame` directly. Interactive sensor submission renders again when the world, physical pose or
optics change. The emulator retains that source between updates. Native scenario
rendering samples each requested interval, holding scenario time fixed until
GPU completion. Submission is bounded to one pending readback and one latest
completed frame; display rendering is independent.

## Assets

Download Crouch End Station's glTF archive through the authorized Sketchfab
Download flow. Download requires a signed-in account; the simulator never
scrapes the viewer or includes account credentials. See its attribution and
`prepare.py --help`. Preparation creates a fresh staging directory and records
source/output SHA-256s. Review scale, orientation and spawn before installing
`environment.json` and `assets/` into that environment's directory. The scene
must have meaningful meter scale before distance/focus tests are meaningful.

The glTF loader imports scene 0 through `WorldAssetRoot` and configures spawned
entities after `WorldInstanceReady`. Imported cameras/lights are disabled.
Photogrammetry can select unlit materials to preserve photographed lighting;
the camera body uses its PBR materials. LCD geometry gets independent UVs and
an unlit texture. No shader is needed for the initial optics implementation.

The web build's `simulation-assets` plugin serves/packages assets directly
from this crate. To open an installed environment, use
`?environment=crouch-end-station`. That URL is retained when selecting firmware.
Large curated GLBs use Git LFS. Source downloads, conversion intermediates and
scenario output stay in ignored build/output directories.

## Build and checks

From the FujiMod workspace, use its Mise toolchain:

```sh
cargo test -p fujimod-simulation --lib
cargo check -p fujimod-simulation --examples
node crates/simulation/build-web.mjs
python3 crates/simulation/environments/crouch-end-station/test_prepare.py
```

The browser app's build/check/test pre-scripts build the Wasm module. Generated
bindings and Wasm live under `bindings/javascript/pkg/simulation/`, never in
source control; the app imports the maintained `fujimod/simulation` package entry.
Use `build-web.mjs --optimize` for Binaryen size optimization when preparing a
release. This step is intentionally omitted from ordinary development checks.

After installing an environment, render a native scenario without a window:

```sh
cargo run -p fujimod-simulation --example render_scenario -- \
  crates/simulation/environments/crouch-end-station/environment.json \
  target/simulation-runs/a-fresh-run
```

`apps/web/tests/simulation_browser.py` exercises the real firmware web app:
Bevy identity, live LCD frames, invariant camera pose during inspection,
walking at real-time speed, and a model control click. It requires the local
firmware preview or an explicit DAT. It does not access a camera/USB device.
GPU image comparisons should use tolerances across adapters; exact pose and
command replay tests do not depend on GPU pixels.

## Implementation references

These are the version-matched upstream patterns used here, rather than older
Bevy tutorials or custom equivalents:

- [Bevy 0.20 migration guide](https://bevy.org/learn/migration-guides/0-19-to-0-20/)
- [glTF loading](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/gltf/load_gltf.rs)
- [glTF material overrides](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/gltf/edit_material_on_gltf.rs)
- [Render targets](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/3d/render_to_texture.rs)
- [GPU readback](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/shader/gpu_readback.rs)
- [Externally driven rendering](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/app/externally_driven_headless_renderer.rs)
- [Depth of field](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/3d/depth_of_field.rs)
- [glTF texture/mipmap preparation](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/large_scenes/mipmap_generator/README.md)
- [Sketchfab Download API](https://sketchfab.com/developers/download-api/downloading-models)
