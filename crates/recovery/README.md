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

`availability(firmware_id)` returns `can_build`, the exact missing facts, output
file names, and the catalog's SD-boot and firmware-update button combinations.
A recovery build requires complete facts, including version-independent
acceptance by the stock updater. `Recipe::build_startup` constructs boot files
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
