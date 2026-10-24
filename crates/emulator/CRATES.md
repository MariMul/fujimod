# Emulator crate layout

Cargo recompiles every crate downstream of a change. The goal is that an edit to
one device model rebuilds that model's crate and the final crates that link it:
under a second for the model's unit tests and two to three seconds for the
release Python extension, with emulation no slower than before.

## Target graph

From the bottom of the graph to the top:

1. `fujimod-emulator-cpu` and its backends.
2. `fujimod-emulator-devices`, the device interface. It holds the `Device`
   trait, bus, `Cx`/`Host`, graph, checkpoint, fingerprint, record, signals,
   work, map, math and the test host. It also holds everything that crosses a
   model boundary:
   - the capability traits sessions and bindings call on devices
     (`capability`);
   - handles that models share with each other and with sessions (pins and
     pads, pulse outputs, card contacts, encoder inputs, lens bridge registers,
     HDMI sinks, XP5 pins and panel clocks, scanout frames, scene and frame
     handles, exposure, focus and aperture handles);
   - the NAND and SD card media that plans, sessions and controllers exchange.

   It contains no device implementation. Two crates sit in the same layer:
   `fujimod-emulator-storage` (host-side FAT services over card media) and the
   image interface `fujimod-emulator-imaging`: scene,
   sensor and image frames and their pixels, the handles and option and spec
   types plans name, image layout, statistics frames, exposure and optics
   declarations, film and packed color, and the EXR II ISP interface. Its
   types are concrete with private fields, except the sensor images and
   exposures an image frame carries, which are `SensorImage` and
   `SensorExposure` trait objects; `SensorDecoders` decodes their checkpoints
   and RAF files. The implementation is two model crates:
   `fujimod-emulator-image-formation` (exposure sampling, optical projection,
   radiometric rendering) and `fujimod-emulator-image-processing` (analysis
   laws, RAW development, resampling, separable filters, RAF reading, the
   scene's optical blur, the clocked frame source and the ISP parameter
   bank). The EXR II ISP input crate registers the frame source and parameter
   bank constructors and the sensor decoders.
3. Plan crates: each family's checked board description and construction
   interface, such as `fujimod-emulator-exr2-plan`. A plan carries each model's
   options as JSON for the model crate to decode, so an option change in a
   model crate never rebuilds the plan. Its `construction` module holds the
   build context, the wiring helpers and the registry of constructors by plan
   model kind. A plan depends on the interface only.
4. Model crates, one per subsystem and about 4,000 lines or fewer, for
   example `fujimod-emulator-exr2-i2c`, `-exr2-display`, `-exr2-gpu`,
   `-parts-lens`. A model crate depends on the interface, its family's plan
   and, only to share behavior, on another model crate. Each family model
   crate's `construct::register(&mut Constructors)` adds the constructors of
   its models; a shared part is wired by the family crate that attaches it
   (the EXR II serial crate wires the lens unit, for example).
5. Board crates: validation, wiring, resources and DMA consolidation.
   `fujimod-emulator-preparation` sits with them: it builds a run's declared
   inputs (NAND media from persistent inputs and settings layouts, cards,
   images, statistics and sensors) into the plan's inputs, so a settings edit
   rebuilds neither the storage interface nor the emulator. They
   construct nodes through the constructor registry and depend on the interface
   and plan crates only.
6. `fujimod-emulator`: execution, scheduling and sessions. It depends on the
   CPU, interface and plan crates and on no catalog crate above the interface's
   own inputs. Sessions reach devices through capability traits, never through
   concrete model types. `fujimod-emulator-boot` sits above it: it prepares
   runs from the reference catalog and run configurations, assembles the CPU
   graph and the X70 plan, and opens sessions.
7. Binding libraries (the Python binding crates, the JavaScript bindings)
   over the emulator and the interface. The Python bindings are one crate per
   area, each depending only on what it binds: `fujimod-python-base` (the
   errors, values, discovery and call snapshots, the XP4 backend hook the
   emulator bindings share, and the board session hook the remote bindings
   borrow a Python board's session through), `-catalog` (the reference catalog and corpus),
   `-service` (App80 service definitions, registered in the catalog module),
   `-usb` (USB, app80, tracing, workflows and firmware files), `-usb-lease`
   (the ff80 session lease the remote bindings borrow from a USB session
   without depending on the USB bindings), `-recording`, `-remote` and
   `-emulator` (boards, run configurations, input assets and settings
   headers). Binding functions that call one family's model code live in that
   family's binding crate (`fujimod-python-graphics` for the graphics data
   path, EXR II GPU and scanout, `-xp4` for XP4 sessions, `-host-image` for
   host image algorithms), which sits with the final crates and registers its
   functions on the module the final crate builds.
8. Final crates of a few lines each: the Python module (`_native`), the CLI
   and worker binaries, and the JavaScript module. Each links everything,
   registers constructors, board builders and family bindings, then starts.
   `fujimod_emulator_models::register()` registers every model crate's
   constructors, so final crates make one call for them.

With this graph, a model edit rebuilds the model crate, the models
registration crate and the final crates. An interface edit rebuilds
everything, so the interface holds declarations and small shared handles, not
behavior that changes often.

## Rules

- Below the final layer, only model crates depend on model crates.
- Emulation speed does not rely on LTO. Small functions called across crates
  on hot paths (bus access, `WordMap`, `Cx`/`Host`, interrupt lines, handle
  accessors) are `#[inline]` or generic. Every step compares host
  instructions retired for the X70 boot and the stock button scenario before
  and after; an increase is fixed before the step lands.
- Tests stay with the code they test. Tests that need models from several
  crates live in the board crate, or in a test crate above the models. Test
  support shared by several model crates goes in the interface's test host or,
  when it installs process-wide state such as a global allocator, in a
  test-only crate (`fujimod-emulator-exr2-isp-testing`).
- Every crate depends on `fujimod-workspace-hack` (`cargo hakari manage-deps`).
- No re-exports, aliases or shims are left for old paths; callers change in the
  same step.
- `crates/crate-graph` checks these layers in the workspace tests: every crate
  has a layer in its `layers.json`, only model and final crates depend on model
  crates, a model crate has at most 4,000 lines outside its tests, and the
  interface implements no device. The violations that remain until the steps
  below are done are listed there; an entry that is no longer true fails, so
  the list only shrinks.

## Capabilities

The interface's `capability` module declares what sessions do with devices:
`AnalogInputs`, `SerialStatus`, `TouchPanel`, `HdmiOutput`, `UsbPort`,
`NandStore`, `CardSlot` and `ScanoutSource`. `Device` has a default accessor
pair per capability (`card_slot()`, `card_slot_mut()`, ...) returning `None`;
a model that offers the capability returns itself. The board's
`inspect_capability`, `edit_capability` and `capability_input` find a node's
capability, and `device_save` reads any device's diagnostic state. Sessions no
longer downcast to models; generic `inspect_device::<T>` remains for tests and
final crates. `FrameSource` configures the clocked frame source's period and
statistics cadence (`StatisticsCadence`, beside it in `capability`). A session
reads RAF input through the sensor decoders the constructor registry holds.

## Board code outside the emulator crate

- XP4 boards live in `fujimod-emulator-xp4-board`, and their Python bindings
  in `fujimod-python-xp4`; the emulator bindings reach an XP4 session through
  `Xp4Backend` in `fujimod-python-base`, whose factory the `_native` module
  registers.
- XP5 boot lives in `fujimod-emulator-xp5-board`: `boot::open` returns the
  engine session with the `session::Inputs` that drive its keys and GPIO
  pins, so the engine session holds no XP5 state.
- These two board crates sit above the emulator, since they open sessions on
  it. Their device assembly still names models. The XP5 resources hold model
  types (mailbox pins and channel, flash storage, display configuration and
  its pixel order) and some kinds are assembled in groups (imaging, general
  DMA, clock banks); the XP4 session drives its ADC, GPIO and USB models by
  type. Each needs those handles in the interface, registered group
  assemblers and capabilities before its assembly can move to a registry.
- X70 boot policy (`boot/x70.rs`) stays only if it names plan and interface
  types alone; otherwise it moves to the EXR II board crate.
- The GIC is a model in `fujimod-emulator-ip-arm` that the A-profile runner
  reaches through `gic::InterruptController` in the interface; board builders
  construct it. The processor parts the runners drive directly live beside
  them: the generic timer and Cortex-A53 implementation registers in
  `board::aprofile`, the ARMv7-M NVIC in `architecture::mprofile`. The
  emulator depends on no model crate.

## Steps

Each step keeps every build and test working, lands as its own commit, and
records the rebuild costs below before and after.

1. Done: final crates. The Python module and the binaries have their own crates.
2. Done: EXR II plans. `fujimod-emulator-exr2-plan` holds plans and resources.
   `fujimod-emulator-exr2-board` constructs devices and is reached through
   `register_board_builder()`.
3. Model crates: done. XP4 and XP5 by subsystem (`-xp4-{storage,display,io}`,
   `-xp5-{imaging,storage,display,io}`), IP (`-ip-arm`, `-ip-dmp`, `-ip`),
   graphics, storage, parts by function (`-parts-{lens,sensor,controls,io}`),
   the EXR II subsystems
   (`-exr2-{display,i2c,gpu,audio,serial,codecs,storage,companion,system,io,boot-serial}`),
   the EXR II ISP (`-exr2-isp-{input,face,statistics,output}`), the image
   codecs (`-image-codecs`) and the sensor input models the XP4 and XP5 boards
   share (`-sensor-input`). Imaging and scene left the devices crate as the
   image layer; X70 REC live view was bit-identical before and after.
4. Plan options, handles and the constructor registry: done for EXR II. Plans
   carry model options as JSON, the handles their resources share are
   interface types, and every EXR II model is built by the constructor its
   crate registers, so neither the plan nor the board depends on a model
   crate. Remaining: XP5 and XP4 device assembly (see above).
5. Capabilities and interrupt delivery: done. The EXR II sessions reach devices
   through capabilities, XP4 boards and XP5 boot left the emulator, and the GIC
   is reached through its interface trait, so neither the emulator nor
   the Python binding libraries depend on a model crate.
6. Bindings: done. The Python binding libraries depend on no model crate;
   XP4 sessions are bound in `fujimod-python-xp4`. The former single
   `fujimod-python` crate is split by area into crates that compile in
   parallel, so a catalog edit no longer waits for every binding to rebuild
   after the emulator.
7. Final link: measured, no change. Relinking the extension module takes
   2.7–3.3 s end to end with the default Apple linker, 3.4–4.2 s with
   `-no_deduplicate` and 3.4–10 s with `lld` (load average 14), so the default
   stays. Release builds carry no debug info.
8. Image interface and implementation: done. The image layer split into the
   interface and its two implementation crates, so the emulator, the plan and
   the Python binding libraries no longer rebuild after an optics, rendering,
   RAW or RAF edit. X70 REC live view stayed bit-identical, with identical
   guest instruction counts, through two seconds after REC.
9. Catalog and its bindings: done. The catalog split by domain into
   `fujimod-catalog-{content,service,construction,controls,remote,settings,bulk}`
   below `fujimod-catalog`, the reference catalog of SoCs, parts, bodies and
   firmware and their hardware profiles. `fujimod-catalog-configuration`
   holds the run configurations above it and `fujimod-catalog-schemas` the
   schema generator. USB, the workflows and the USB bindings depend on the
   service crate only, and the EXR II plan and models on the construction
   crate. The service definition bindings (`fujimod-python-service`) and the
   ff80 session lease (`fujimod-python-usb-lease`) left the catalog and USB
   bindings. The catalog-driven boot moved from the emulator into
   `fujimod-emulator-boot`, and the run input preparation from the emulator
   and the storage interface into `fujimod-emulator-preparation`, so the
   emulator depends on no catalog crate except the content and construction
   crates and no settings crate. The X70 boot to REC ran the same 48,023,300
   guest instructions after both moves, with the same host instructions
   retired. The remote bindings borrow a board's session through
   `fujimod-python-base`, so they no longer wait for the emulator bindings.

## Rebuild costs

Wall seconds after one edit: the release Python extension
(`scripts/python_extension.py`), and the edited crate's unit tests
(`cargo test -p` with `--no-run`). "Before step 2" was measured on a loaded
machine. "Before steps 6 and 8" precedes the binding split of step 6 and the
image split of step 8: the catalog and image rows were measured in the same
checkout as the next column, at load average 5–11, and the other rows
repeat the measurements before them (load average 4–8). "Before step 9"
precedes the catalog split, when every catalog row was one crate edit.
"After the domain split" was measured after the catalog and binding splits of
step 9, before the boot, preparation and configuration moves. The "now"
columns were measured after all of step 9 at load average 3–6, two edits per
row; a catalog row edits the file named, otherwise the crate's `lib.rs`.

| Edit | Extension before step 2 | Before steps 6 and 8 | Before step 9 | After the domain split | Extension now | Unit tests now | Target |
|---|---|---|---|---|---|---|---|
| EXR II I2C controller | 23–26 | 1.4 | 1.4 | 1.3–1.4 | 1.4 | 0.7 | 2–3; under 1 |
| EXR II ISP image input | 15–19 | 1.6 | 1.4 | 1.4–1.5 | 1.3–1.4 | 0.8 | 2–3; under 1 |
| Lens part | 23–26 | 2.0 | 1.8 | 1.7–1.8 | 1.7–1.8 | 0.7 | 2–3; under 1 |
| XP4 I/O | | 2.0–2.1 | 1.6 | 1.6–1.7 | 1.6–1.7 | 0.7 | 2–3; under 1 |
| XP5 I/O | | not linked | not linked | not linked | not linked | 0.7 | under 1 |
| Arm interrupt controller | | 3.3–3.5 | 1.5 | 1.5 | 1.5 | 0.6 | 2–3; under 1 |
| Image formation | | 6.5–7.8 | 2.3–4.6 | 1.7–1.8 | 1.7–1.9 | 0.7 | 2–3; under 1 |
| Image processing | | 6.5–7.8 | 2.1–2.2 | 1.6–1.7 | 1.6–1.7 | 0.6 | 2–3; under 1 |
| Image interface | 23–26 | 6.5–7.8 | 4.7–5.2 | 4.2–4.6 | 3.7–4.4 | 0.8 | |
| EXR II plan | | 12.7–17 | 6.0–7.7 | 3.7–3.8 | 3.4 | 0.8 | |
| Boot assembly | | | | | 2.0 | 1.3 | 2–3 |
| Run input preparation | | | | | 2.1–2.5 | 0.6 | 2–3 |
| Run configuration (`lib.rs`, was `configuration.rs`) | | | 8.1–9.6 | 5.0–6.0 | 2.7 | 1.3 | 2–3 |
| Reference catalog (`reference.rs`) | | 9.7–10.6 | 8.1–9.6 | 5.1–6.2 | 3.8 | 1.3 | 2–3 |
| Hardware profiles (`profiles.rs`) | | | 8.1–9.6 | | 3.8–3.9 | 1.3 | 2–3 |
| Catalog content | | | 8.1–9.6 | 5.9–6.9 | 4.5–5.1 | 0.5 | 2–3 |
| Catalog service | | | 8.1–9.6 | 3.9–4.1 | 3.9–4.1 | 0.6 | 2–3 |
| Catalog construction | | | 8.1–9.6 | 5.3–5.4 | 4.5–5.1 | 0.6 | 2–3 |
| Catalog controls (`facts.rs`) | | | 8.1–9.6 | 5.1–5.2 | 4.1–4.2 | 0.7 | 2–3 |
| Catalog remote | | | 8.1–9.6 | 5.3–6.0 | 3.9 | 0.6 | 2–3 |
| Catalog settings | | | 8.1–9.6 | 7.6–8.0 | 3.9 | 0.5 | 2–3 |
| Catalog bulk | | | 8.1–9.6 | 5.2–5.8 | 4.0–6.6 | 0.5 | 2–3 |

An EXR II model edit recompiles the model crate (0.2–0.3 s),
`fujimod-emulator-models` (0.1 s) and the extension module (0.8–1.0 s, mostly
linking). Before the registry it also recompiled the board crate (0.4–0.5 s;
2.1–2.3 s in all for the I2C controller, 2.6 s for the lens part). A lens part
edit also recompiles the EXR II serial, audio and boot serial crates, which
depend on it. The extension links no XP5 model. An Arm interrupt controller
edit recompiles the XP4 board and its Python bindings with the extension
module; before the GIC trait it also recompiled the emulator and all Python
bindings.

Before step 8 an image formation or processing edit was an image layer edit:
it rebuilt the plan, the emulator, every EXR II model and `fujimod-python`.
It now recompiles the edited crate (0.2–0.3 s; 1.4 s for the first formation
edit), the ISP, codec and sensor models that use it, the host image bindings
and the module. An image interface edit still rebuilds the plan (0.9 s), the
emulator (2.0–2.1 s), the EXR II models in parallel and the emulator bindings
(0.7 s); a plan edit adds the emulator (1.4 s now) and the emulator and remote
bindings.

Before step 9 a catalog edit rebuilt the 20.8K-line catalog (2.6–3.5 s), then
`fujimod-usb` (1.0–1.3 s), `fujimod-python-usb` (2.5–3.2 s) and the remote
bindings on its critical path, beside the plan, every EXR II model and the
emulator. A run configuration edit now rebuilds the configuration crate
(0.8–0.9 s), the boot crate (0.6–0.8 s), the catalog and emulator bindings
(0.3–0.5 s) and the module (0.8 s). A reference catalog or hardware profile
edit adds `fujimod-catalog` (1.1–1.2 s for its 7.7K lines) before the
configuration crate, and a document crate edit adds that crate (0.1–0.3 s)
and, for controls, the remote crate. The remote bindings (0.6 s) build beside
the configuration and boot crates. A content edit also rebuilds the emulator
(0.8 s), the workflows and the USB bindings (2.4 s) beside that path, and a
construction edit the plan, the EXR II models and the emulator. A service edit
rebuilds `fujimod-usb` (0.5 s), the USB bindings (2.0 s) and the module only.
The reference and configuration unit tests rebuild in 1.3 s; each document
crate's own tests in 0.5–0.7 s.

The remaining catalog chain is the reference catalog, the configuration
crate, the boot crate and the emulator bindings in series: each needs the
types of the one before, and Cargo starts none of them before the previous
crate finishes.
