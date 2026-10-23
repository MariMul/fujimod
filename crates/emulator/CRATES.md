# Emulator crate layout

Cargo recompiles every crate downstream of a change. An edit to one device
model should rebuild that model's crate, the crates that construct it, and a
final crate of a few lines, not the emulator, its bindings or unrelated models.
Frequently edited code therefore sits near the leaves of the crate graph, and
the large shared crates depend only on stable interfaces.

## Layers

From the bottom of the graph to the top:

1. `fujimod-emulator-cpu` and its backends.
2. `fujimod-emulator-devices`, the device interfaces: the `Device` trait, bus,
   `Cx`/`Host`, graph, checkpoint, fingerprint, signals, work, records, the
   shared EXR II ISP interfaces, the test host, and the handle types that boards
   and sessions exchange with models (pins, levels, analog inputs, card media,
   scene handles). It contains no device models.
3. Model crates, one per family or topic (parts, imaging, graphics, IP blocks,
   storage, EXR II, EXR II ISP, XP4, XP5). Each depends on the interface crate
   and on the model crates it builds on.
4. Plan crates: each family's checked board description. Model options stay
   JSON until the model crate decodes them, so a model's options can change
   without rebuilding the plan.
5. Board assembly crates: construct models from a plan.
6. `fujimod-emulator`: execution, sessions and boot orchestration, depending on
   the interface and plan crates. Board assembly reaches it through assemblers
   installed at startup.
7. Bindings as libraries over the emulator.
8. Final crates of a few lines each: the Python module (`_native`), the CLI and
   worker binaries, and the JavaScript module. Each depends on everything it
   links and installs the board assemblers before use.

A crate below the final layer depends on a model or assembly crate only when it
constructs that model. Tests that need assembled boards live in, or depend on,
an assembly crate.

Cargo cannot tell an edit to a model's behavior from an edit to its interface,
so anything that names a model's options rebuilds with it. A model's options,
wiring and shared handle types therefore live in the interface layer (or a
small options crate per family), named by plans, boot code and sessions; the
implementation crate is named only by assembly and final crates.

## Stages

Each stage keeps every build and test working and records the cost of a device
edit before and after.

1. Final crates: the Python module and the binaries move into their own crates.
2. EXR II: the ISP options, wiring and handle types the board names move into
   the devices crate's `exr2::isp` interface module. `fujimod-emulator-exr2-board`
   splits into a plan crate (`Plan`, `Resources`, `Wire`, validation) and the
   assembly crate (`construct`, DMA consolidation, building an `Assembly`).
   `Plan::build` calls the assembler the final crates install, so an ISP edit
   rebuilds the ISP crate, the assembly crate and the final crates only.
3. Models leave `fujimod-emulator-devices` family by family, XP4 and XP5 first,
   imaging, scene and ISP last.
4. Binding functions that call model code directly move next to those models.
