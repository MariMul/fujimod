# SD recovery cards

`fujimod-recovery` constructs SD startup files from an official update supplied
by the caller. It checks the complete DAT identity, decoded source hashes,
initialized mappings, BSS, expected old words, header policy, and file names.
The official DAT is included unchanged at its required card-root name.

The processor selects the packaging mechanism. EXR II uses the stock NAND
header fragments with SD power hold and a preloaded RAM image. XP4 uses raw
FFINIT with the update's DDR setup and relocated tables, the Topaz filesystem
ABI, and a cold preloaded RAM image. Reset mode remains cold; three checked
comparisons select the preloaded table, images, and worker bypass.

XP5 preserves the copied setup prefix, retains its stock DDR initialization,
then loads FAT32 `FFBOOT.BIN` and checks its exact length and CRC32 before
publishing readiness to the secondary-core gate. Ordered SD host, power, and
clock effects come from catalog inputs pinned to source spans. Flash-linked
setup tails and missing board routing remain explicit missing facts.

`availability(firmware_id)` returns `can_build`, the exact missing facts, output
file names, and the catalog's SD-boot and firmware-update button combinations.
A recovery build requires complete facts, including version-independent
acceptance by the stock updater. Located stock getters and producers supply the
SD-running firmware's own public version; the stock comparison accepts equality,
so the paired official DAT needs no version change. The recipe pins those
code spans and verifies them against the constructed RAM image. `Recipe::build_startup` constructs boot files
separately for execution and transfer tests.

JavaScript uses `recovery.availability(firmwareId)` and
`recovery.build(firmwareId, datBytes)`. Call `card.takeFiles()` to obtain file
objects with `name`, `size`, and `bytes: Uint8Array`. Free the file objects and
card after copying their contents to downloads or an archive. Construction has
no filesystem, USB, camera, or network access.

The native CLI exposes `list`, `build FIRMWARE DAT NEW_DIRECTORY`, and
`startup FIRMWARE DAT NEW_DIRECTORY`.

The guest loader sources are in `guest/`. Its checked-in generated Rust
template contains the compiled loader and relocations, with no update or ROM
data. To regenerate it, set `FFINIT_CC` to GNU GCC 4.6.4 and `FFINIT_LD` to GNU
ld, then run:

```sh
cargo run -p fujimod-recovery --example generate_ffinit
cargo run -p fujimod-recovery --example generate_ffinit -- --check
```

The generated template preserves the loader's declaration and link order.
Runtime relocation inserts the supplied update's setup tables and updates all
absolute references, transfer size, CRC32, workspace, and filesystem calls.

The XP5 template is regenerated with GNU AArch64 GCC 9.2.1 and GNU ld:

```sh
XP5_CC=/path/to/aarch64-none-elf-gcc XP5_LD=/path/to/aarch64-none-elf-ld \
  cargo run -p fujimod-recovery --example generate_xp5
```

The XP5 generator rejects unsupported relocations and requires GCC 9.2.1.
Append `-- --check` to compare it with the checked-in template. Both template
generators use empty camera-data placeholders; card construction needs only
the official DAT and the compiled catalog recipes.
