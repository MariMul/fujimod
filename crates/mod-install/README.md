# Debug mod installation helpers

`fujimod-mod-install` composes the ff80 debug and bulk transports for installation.
Its Rust implementation is shared by native Python and JavaScript/WebUSB,
including Pyodide, over local USB or USB sharing.

- `verified_writes` builds bounded write/readback batches. Each packet's guard
  runs at the USB owner before later writes; batches contain at most 512 commands.
- `BorrowedQueries` checks the original callback and output bytes, verifies each
  setup write, runs callbacks with explicit accepted status pairs, and restores
  the borrowed words once after the batch. Calls return reply/output indices.
- `OwnedUpload` checks that the image and its padded bulk extent fit inside the
  supplied owned allocation. `upload` checks the original destination pointer
  and capacity, borrows the pointer, sends E0 pieces, checks completion and
  restores the pointer. `verify` compares every image byte through an explicit
  qualified E1 window and fences the session on mismatch.

The caller qualifies firmware instructions and address views, establishes actual
allocation ownership, authorizes callback targets, maintains caches and verifies
the image before publishing hooks or executing initializers. The crate does not
infer any of these from a model name. Guards are not atomic against device tasks.
Errors and cancellation never retry, repair pointers, free allocations or reset
the device. Retain ownership records and memory when completion is uncertain.

```rust,no_run
use fujimod_mod_install::{OwnedUpload, UploadDestination};
use fujimod_usb::{Result, ff80::{Session, commands::memory::{DebugMemoryLimits,
    MemoryRange}}, operations::data::DataWindow};
use std::num::NonZeroU32;

async fn transfer(session: &mut Session, allocation: MemoryRange, image: Vec<u8>,
    destination: UploadDestination, limits: DebugMemoryLimits, readback: DataWindow,
    piece_bytes: NonZeroU32, alignment: NonZeroU32) -> Result<()> {
    let upload = OwnedUpload::new(allocation, allocation.address(), image,
                                  piece_bytes, alignment)?;
    upload.upload(session, destination, limits).await?;
    upload.verify(session, readback).await?;
    Ok(())
}
```

Bindings expose `usb.InstallQueries`, `usb.OwnedUpload`,
`usb.CommandSequence.verified_writes` / `verifiedWrites`, and
`session.upload_owned` / `uploadOwned`, `verify_owned` / `verifyOwned`.
Firmware and allocation policy stays with the caller in each language.

Run `cargo test -p fujimod-mod-install` for range, padding, packet/batch bounds,
pointer guards, bulk completion, cancellation and full readback checks through
the ff80 core. These checks do not measure physical camera throughput.
