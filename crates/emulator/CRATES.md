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
   - the capability traits sessions and bindings call on devices;
   - handles that models share with each other and with sessions (pins and
     pads, pulse outputs, scanout frames, scene and frame handles, exposure,
     focus and aperture handles);
   - every model option type a plan names.

   It contains no device implementation.
3. Model crates, one per subsystem and about 4,000 lines or fewer, for
   example `fujimod-emulator-exr2-i2c`, `-exr2-display`, `-exr2-dma`,
   `-exr2-gpu`, `-parts-lens`, `-imaging`, `-xp4`, `-xp5`. A model crate
   depends on the interface and, only to share behavior, on another model
   crate. Each exports `register(&mut Constructors)`, which adds its
   constructors by model kind.
4. Plan crates: each family's checked board description, such as
   `fujimod-emulator-exr2-plan`.
5. Board crates: validation, wiring, resources and DMA consolidation. They
   construct nodes through the constructor registry and depend on the interface
   and plan crates only.
6. `fujimod-emulator`: execution, scheduling, sessions and boot orchestration.
   It depends on the CPU, interface and plan crates. Sessions reach devices
   through capability traits, never through concrete model types.
7. Binding libraries (`fujimod-python`, the JavaScript bindings) over the
   emulator and the interface. Binding functions that call one family's model
   code (GPU rendering, display hashes, NAND overlays, FAT images, RAF reading)
   live in that family's binding crate.
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
  crates live in the board crate, or in a test crate above the models.
- Every crate depends on `fujimod-workspace-hack` (`cargo hakari manage-deps`).
- No re-exports, aliases or shims are left for old paths; callers change in the
  same step.

## Capabilities

Sessions currently downcast to concrete models through `Host::inspect_device`,
`edit_device` and `device_input`: `SdHost`, `Scanout`, `SpiController`,
`I2cController`, the ADC `Converter`, `NandController`, `UsbController`,
`InterruptController`, `ClockedFrames`, the HDMI `Transmitter` and sink input,
`Timer`, `PendingDma` and `DeadlineSample`. Each use becomes an interface trait
named for what the session does with it (card slot, scanout source, key scan,
and so on). `Device` gains one default method per capability returning
`Option<&mut dyn Capability>`, which models override. Sessions find nodes by
role and call capabilities. Generic `inspect_device::<T>` remains only for
tests and final crates.

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
2. Done: EXR II plans. `fujimod-emulator-exr2-plan` holds plans, resources
   and the ISP option types. `fujimod-emulator-exr2-board` constructs devices
   and is reached through `register_board_builder()`.
3. Model crates. Shared handles and options move into the interface, ending
   the current cycles:
   - `parts` ↔ `exr2::pulse`/`exr2::gpio`;
   - `parts` → `imaging` → `scene` → `parts`;
   - `bus` → `graphics::scanout`.

   Then the families leave `fujimod-emulator-devices` in this order: XP4,
   XP5, IP, graphics, storage, parts, imaging with scene, and the EXR II
   subsystems.
4. Constructor registry. Construction code moves from the board crates into
   the model crates. Board crates and the emulator stop depending on models.
5. Capabilities. Sessions, XP4 and XP5 board code and interrupt delivery are
   inverted as above, and the emulator depends on no model crate.
6. Bindings. Family binding crates take the model-specific Python functions,
   and `fujimod-python` depends on no model crate.
7. Final link. Measure the linker for the extension and test binaries: Apple
   ld flags such as `-no_deduplicate`, `lld`, and debug info settings. Keep
   whatever is fastest.

## Rebuild costs

Release Python extension (`scripts/python_extension.py`), wall seconds on a
loaded machine, after one edit:

| Edit | Before step 2 | After step 2 | Target |
|---|---|---|---|
| EXR II ISP model | 15–19 | 4.4–7.0 | 2–3 |
| Model inside `fujimod-emulator-devices` | 23–26 | 23–26 | 2–3 |
| Model unit tests (`cargo test -p` the model's crate) | | | under 1 |
