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
   `fujimod-emulator-storage` (FAT and media inputs, host-side services over
   those media) and the image layer `fujimod-emulator-imaging` (scene frames,
   exposure, optics, radiometry, image analysis, film, RAW development and color
   processing, and the EXR II ISP interface), whose types are concrete with
   private fields.
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
5. Board crates: validation, wiring, resources and DMA consolidation. They
   construct nodes through the constructor registry and depend on the interface
   and plan crates only.
6. `fujimod-emulator`: execution, scheduling, sessions and boot orchestration.
   It depends on the CPU, interface and plan crates. Sessions reach devices
   through capability traits, never through concrete model types.
7. Binding libraries (the Python binding crates, the JavaScript bindings)
   over the emulator and the interface. The Python bindings are one crate per
   area, each depending only on what it binds: `fujimod-python-base` (the
   errors, values, discovery and call snapshots and the XP4 backend hook the
   emulator bindings share), `-catalog` (catalog and corpus), `-usb` (USB,
   app80, tracing, workflows and firmware files), `-recording`, `-remote` and
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
longer downcast to EXR II models; generic `inspect_device::<T>` remains for
tests and final crates. `ClockedFrames` keeps typed access because the image
layer that holds it is interface-level.

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

## Rebuild costs

Wall seconds after one edit: the release Python extension
(`scripts/python_extension.py`), and the edited crate's unit tests
(`cargo test -p` with `--no-run`). "Before step 2" was measured on a loaded
machine, the current columns at load average 4–8.

| Edit | Extension before step 2 | Extension now | Unit tests now | Target |
|---|---|---|---|---|
| EXR II I2C controller | 23–26 | 1.4 | 0.7 | 2–3; under 1 |
| EXR II ISP image input | 15–19 | 1.6 | 0.9 | 2–3; under 1 |
| Lens part | 23–26 | 2.0 | 0.8 | 2–3; under 1 |
| XP4 I/O | | 2.0–2.1 | 0.8 | 2–3; under 1 |
| XP5 I/O | | not linked | 0.8 | under 1 |
| Arm interrupt controller | | 3.3–3.5 | 0.8 | 2–3; under 1 |
| Image layer (interface) | 23–26 | 8.6–12.6 | 1.1 | |
| EXR II plan | | 12.7–17 | 1.0 | |

An EXR II model edit recompiles the model crate (0.2–0.3 s),
`fujimod-emulator-models` (0.1 s) and the extension module (0.9–1.0 s, mostly
linking). Before the registry it also recompiled the board crate (0.4–0.5 s;
2.1–2.3 s in all for the I2C controller, 2.6 s for the lens part). A lens part
edit also recompiles the EXR II serial, audio and boot serial crates, which
depend on it. The extension links no XP5 model. An Arm interrupt controller edit recompiles the
XP4 board and its Python bindings with the extension module; before the GIC
trait it also recompiled the emulator and all Python bindings. Interface and plan
edits rebuild their dependents across the workspace.
