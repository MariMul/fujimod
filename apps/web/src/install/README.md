# SD-card dual-boot installer

`Page.tsx` is the installer UI; `controller.ts` owns the ordered workflow and
verification. The camera/firmware-specific instructions live in
`devices/x70/installer.ts`. Only X70 1.10 currently has a reviewed profile.

The hardware adapter is deliberately unavailable. The browser USB library has
no verified SD filesystem probe, FAT32 formatter or file-transfer operation for
this flow yet. In particular, app80 `format_storage` is not an SD-card FAT32
formatter and must not be used as one. No camera is accessed by this page.

For UI review only, development URLs `/install/?preview=exfat` and
`/install/?preview=fat32` use an explicitly labeled in-memory test adapter.
Its sample files are not bootable firmware; it does not emulate firmware or
assert hardware behavior. The production route does not select that adapter.
The real connection action stays disabled until a hardware adapter is supplied.

The manual formatting path leaves the SD card in the camera and directs the user
to its Format menu. After reconnecting, the installer probes the filesystem again;
formatting in the camera does not bypass the FAT32 requirement.

## Adapter requirements

- Identify the exact connected camera and firmware, and whether the current boot
  is debug-capable and service/JIG or ordinary REC. A normal REC debug session
  must not write the JIG guard.
- In service mode, arm and verify the complete guarded exit preflight in
  `research/service-exit/README.md` before SD access and immediately before any
  instruction to remove the battery/disconnect. `guard()` must verify unchanged
  settings and return only after successful cleanup; it must never clear `a3`.
- Probe physical card identity, actual filesystem, capacity and free space.
  Keep that identity stable across formatting and reject card replacements.
- Bind the two installation files and their source SHA-256 hashes to the exact
  firmware identity. The controller checks both source hashes before any write.
- Format only the selected SD card, and only after explicit erase consent.
  Return after durable completion; the controller probes FAT32 again.
- Write `ffboot.bin` and `ffsys.bin` at the card root. Finish/flush each write
  before returning. Reads must come from the card after reopening the files,
  never a host cache or upload buffer. The controller compares lengths and
  SHA-256 digests of the read-back bytes.
- A disconnect or uncertain operation must stop progression. Never retry a
  writer, format operation, mode request or cleanup automatically. `abandon()`
  releases host resources without camera writes or clearing the guard.

The page requests the owner's battery-removal sequence after verified transfer
and guard checks, then presents the model-specific dual-boot buttons. Completion
requires the user to press MENU on the camera and explicitly confirm seeing
FujiMod. It does not claim an automatic post-boot firmware identification.

Tests cover ordering, explicit erase consent, wrong firmware, guard failures,
source/read-back hash mismatch, card swaps, falsely successful formatting,
in-camera formatting and filesystem rechecks, disconnection and repeated clicks.
They use the test adapter; they do not establish hardware safety or execute stock firmware.
