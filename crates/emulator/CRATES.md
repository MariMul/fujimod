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

   It contains no device implementation. `fujimod-emulator-storage` (FAT and
   media inputs, host-side services over those media) sits in the same layer.
3. Model crates, one per subsystem and about 4,000 lines or fewer, for
   example `fujimod-emulator-exr2-i2c`, `-exr2-display`, `-exr2-dma`,
   `-exr2-gpu`, `-parts-lens`, `-imaging`, `-xp4`, `-xp5`. A model crate
   depends on the interface and, only to share behavior, on another model
   crate. Each exports `register(&mut Constructors)`, which adds its
   constructors by model kind.
4. Plan crates: each family's checked board description, such as
   `fujimod-emulator-exr2-plan`. A plan carries each model's options as JSON;
   the board crate decodes them for the model, so an option change in a model
   crate never rebuilds the plan.
5. Board crates: validation, wiring, resources and DMA consolidation. They
   construct nodes through the constructor registry and depend on the interface
   and plan crates only.
6. `fujimod-emulator`: execution, scheduling, sessions and boot orchestration.
   It depends on the CPU, interface and plan crates. Sessions reach devices
   through capability traits, never through concrete model types.
7. Binding libraries (`fujimod-python`, the JavaScript bindings) over the
   emulator and the interface. Binding functions that call one family's model
   code live in that family's binding crate (`fujimod-python-graphics` for the
   graphics data path, EXR II GPU and scanout), which sits with the final
   crates and registers its functions on the module the final crate builds.
8. Final crates of a few lines each: the Python module (`_native`), the CLI
   and worker binaries, and the JavaScript module. Each links everything,
   registers constructors, board builders and family bindings, then starts.
   `fujimod-emulator-models::register()` registers every model crate, so final
   crates make one call.

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
tests and final crates. `ClockedFrames` and the XP5 session controls still use
typed access until imaging and scene, and XP5 boot, leave the engine.

## Board code that leaves the emulator crate

- XP4 boards (`board/xp4`) move to an XP4 board crate.
- XP5 boot (`boot/xp5.rs`, `session/xp5.rs`) moves to an XP5 board crate.
- X70 boot policy (`boot/x70.rs`) stays only if it names plan and interface
  types alone; otherwise it moves to the EXR II board crate.
- ARM interrupt controllers and timers (`ip::arm`) become model crates. The
  emulator delivers interrupts through interface traits, and board builders
  construct the controllers.

## Steps

Each step keeps every build and test working, lands as its own commit, and
records the rebuild costs below before and after.

1. Done: final crates. The Python module and the binaries have their own crates.
2. Done: EXR II plans. `fujimod-emulator-exr2-plan` holds plans and resources.
   `fujimod-emulator-exr2-board` constructs devices and is reached through
   `register_board_builder()`.
3. Model crates. Done: XP4 and XP5 by subsystem (`-xp4-{storage,display,io}`,
   `-xp5-{imaging,storage,display,io}` and the board-layer `-xp5-board`), IP
   (`-ip-arm`, `-ip-dmp`, `-ip`), graphics, storage, parts by function
   (`-parts-{lens,sensor,controls,io}`), the EXR II subsystems
   (`-exr2-{display,i2c,gpu,audio,serial,codecs,storage,companion,system,io}`)
   and the EXR II ISP (`-exr2-isp-{input,face,statistics,output}`). Remaining:
   imaging with scene, on the boundary agreed with their maintainers (frame
   types with their CFA and radiance stay in the interface; mosaic sites and
   other processing move to the imaging crate, cached by frame identity).
4. Plan options and handles: done. Plans carry model options as JSON and the
   handles their resources share are interface types, so the plan depends on
   no model crate. Remaining: the constructor registry, after which board crates
   stop depending on models.
5. Capabilities: done for the EXR II sessions, so the emulator no longer
   depends on the EXR II display, I2C, serial, I/O or storage crates.
   Remaining: XP4 and XP5 board code and interrupt delivery.
6. Bindings: done. `fujimod-python` depends on no model crate.
7. Final link. Measure the linker for the extension and test binaries: Apple
   ld flags such as `-no_deduplicate`, `lld`, and debug info settings. Keep
   whatever is fastest.

## Rebuild costs

Wall seconds on a loaded machine after one edit: the release Python extension
(`scripts/python_extension.py`), and the edited crate's unit tests
(`cargo test -p` with `--no-run`).

| Edit | Extension before step 2 | Extension now | Unit tests now | Target |
|---|---|---|---|---|
| EXR II I2C controller | 23–26 | 2.9–3.5 | 1.5 | 2–3; under 1 |
| Lens part | 23–26 | 3.9–4.4 | 1.5 | 2–3; under 1 |
| EXR II ISP model | 15–19 | about 3 | about 1.5 | 2–3; under 1 |
| Imaging or scene (still in the devices crate) | 23–26 | 18 | 3.5 | 2–3; under 1 |

An EXR II model edit now recompiles the model crate (0.4–0.7 s), the board
crate (0.6–0.7 s) and the extension module (1.4–1.9 s, mostly linking).
