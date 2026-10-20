# Camera workflows

`fujimod-workflows` holds camera procedures composed from `fujimod-usb`
operations (design: [`../WORKFLOWS.md`](../WORKFLOWS.md)). Its Rust
implementation is shared by native Python and JavaScript/WebUSB, including
Pyodide, over local USB or USB sharing.

## `debug_mode`: persistent debug access

`debug_mode::enter` / `leave` store the selected camera definition's debug
byte as 1 / 0. Both arm the service reset guard, verify code and storage
geometry, capture the logical storage banks and configuration RAM, and check
the borrowed adapter with a no-write probe before calling the one-byte range
writer once. Readback must differ only at the debug byte; stored reset guard,
settings/profile payloads, scratch and callbacks must remain unchanged.

Successful cleanup leaves debug access off and the reset guard armed in RAM.
Failure restores the original debug flag when completion is known, preserves
unexpected callback/scratch values, and fences failed cleanup. No writer is
retried. Python exposes `Session.enter_debug_mode` / `leave_debug_mode` with
the same camera table and firmware identity arguments as `arm_reset_guard`;
they return `(already_persisted, progress_lines)`.

## `install`: debug mod installation

Installation also uses two generic `fujimod-usb` operations:
`operations::verified::verified_writes` (bounded write/readback batches whose
guards run at the USB owner) and `operations::borrowed::BorrowedQueries`
(borrowed callback words, explicit accepted statuses, one restore per batch).

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
use fujimod_workflows::install::{OwnedUpload, UploadDestination};
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

Run `cargo test -p fujimod-workflows` for range, padding, packet/batch bounds,
pointer guards, bulk completion, cancellation and full readback checks through
the ff80 core. These checks do not measure physical camera throughput.
