# Recovery qualification facts

`flash-free.json` records completed flash-free updater qualifications by
firmware ID. An absent, incomplete, or stale fact keeps `can_build` false.
The record format is `FlashFreeProof` in the recovery crate.

Qualification requires continuous strict execution from verified ROM reset
with the SD-boot combination and DISP/BACK held. Both zeroed and scrambled
update-covered internal storage must reach the card updater's DAT-read point
with zero internal code or update-data reads. Settings outside the update's
write set may remain. Entire-storage corruption results are reported separately.

The construction digest is SHA-256 of the recipe serialized as a JSON value
with sorted object keys, omitting `flash_free` and `missing`. Records also pin
the ROM, emulator, configuration, execution receipts and every emitted file.
Construction verifies the final file bytes against those identities before
returning a recovery card. Startup diagnostics do not establish qualification.
