# Native emulator

`fujimod-emulator` boots a catalog body and firmware from files you provide.
The initial runtime supports **X70 / x70-110**. It runs the real boot ROM,
NAND loader and firmware with strict native device models. Unsupported hardware
stops execution and remains visible in the JSON report.

Build the [Unicorn backend](../emulator-cpu-unicorn/README.md), then:

```sh
cargo build --locked --release -p fujimod-emulator --features serde \
  --bin fujimod-emulator
target/release/fujimod-emulator boot \
  --body x70 --firmware x70-110 \
  --update /path/to/FPUPDATE.DAT \
  --boot-rom /path/to/bootrom.bin \
  --unicorn "$PWD/target/unicorn-native/libunicorn.2.dylib" \
  --screenshot /path/to/lcd.png
```

Use the shared library filename produced on your platform. Firmware and ROM
size and SHA-256 must match the selected catalog entry. Supplying your own
16 KiB boot ROM dump is required; the command never fetches one. Binary input
paths may be absolute or relative to the current directory.

The default NAND contains the selected update and erased settings space.
The RTC starts at 2000-01-01 and the external image is fixed gray. These are
declared inputs, not camera calibration. First-start language/date pages are
accepted through calibrated MENU/OK ADC presses on the guest timeline. The
report distinguishes normal REC (`system_mode` 1) and first-start REC (3).
It includes pending-operation state and never counts a discovery run as a boot.

Optional persistent inputs:

- `--nand IMAGE`: a committed NAND JSON image exported by the emulator, or a
  complete 128 MiB raw medium whose firmware prefix matches the selected update.
  The committed format verifies geometry and firmware base identity.
- `--settings MCU.BIN`: the 16-byte MCU persistent packet, including marker
  byte `0xaa` at offset 14 and an eight-bit zero sum. User settings stored in
  NAND belong in the NAND image.
- `--instruction-limit COUNT`: a positive instruction budget, default two billion.

The command prints a JSON report and exits 0 at REC, 2 at an instruction limit
or execution frontier, and 1 for invalid inputs or screenshot errors. A requested
PNG captures the currently composed LCD without advancing the guest. Input
files are read only; firmware-written settings remain in the session's medium.

Rust embedding uses `fujimod_emulator::boot::{Options, open, to_rec, screenshot}`.
`open` returns an unexecuted `session::Session`; its `board` owns execution and
its other methods supply external inputs and passive observations. `to_rec`
returns a stopped report while retaining the session for inspection. No Python
runtime is required.
