# FujiMod Python package

This directory builds the `fujimod` Python distribution. PyO3 source lives in
`src/`; the USB implementation lives in the workspace's `crates/usb/`.
There is one package and no separate top-level Python source tree.

Build from the repository root with maturin:

```sh
maturin build --manifest-path bindings/python/Cargo.toml --out target/wheels
```

Install the wheel in the consuming project's uv environment. Source consumers
can use `uv add /path/to/fujimod/bindings/python` and rebuild after Rust changes.

```python
from fujimod import usb
from fujimod.usb import blocking

# Supply these from the selected body/firmware definition, not global defaults.
def identify(device, interface, firmware_limits):
    session = blocking.Session.open(device, interface, firmware_limits)
    try:
        return session.device_info(usb.InfoSelector.Product)
    finally:
        if session.state == usb.SessionState.Ready:
            session.close()
```

`blocking.discover(usb.DeviceFilter.Diagnostic)` returns device descriptors;
`device.interfaces()` inspects available bulk interface selections. The caller
selects the device and interface explicitly. `DeviceFilter.Fujifilm` includes
other Fujifilm USB modes for inventory, without opening a protocol session.

All wire exchanges run through the async Rust implementation. This native
blocking adapter releases Python's GIL during device I/O and serializes access
to each session. Commands accept enums for selectors, views, modes and key
states. Numbers represent addresses, lengths and raw device fields. Research
queries require `usb.DiagnosticTag.research(value)` explicitly.

`Session.read_memory` and `Session.write_memory` issue one camera request each.
`blocking.memory.read_range` and `blocking.memory.write_range` use the library's
composed operations. `blocking.memory.read_chunks` returns a context-managed
iterator that exclusively owns the session until iteration finishes or closes.
Each yielded buffer can be saved immediately; failures retain the prior chunks.
Configuration writes
are distinct from persistent saves. `diagnostic_query`, `prepare_data_read`,
`prepare_data_write`, `read_data`, `write_data` and `empty_response` expose separate
stages without implicit preparation, firmware retries or cleanup.

Camera errors have a separate exception type from protocol/transport failures.
Device-command exceptions retain `command`, `primary` and `secondary` fields;
failed transfers retain available partial bytes. Diagnostic replies expose a
`CommandStatus` enum alongside raw fields, and console polls expose a
`ConsoleProgress` enum, preserving unknown progress values. No integer return
value is interpreted as an instruction to wait.

An incomplete exchange leaves the session in `CompletionUnknown`, where further
I/O is rejected. `close()` consumes the session on both success and failure:
`Closed` means its two-byte C4 reply was received; a failed or malformed close
leaves `CompletionUnknown` and cannot be retried. The two reply bytes remain
opaque and do not certify a camera power cycle. Releasing Python objects does
not send a camera reset, mode transition or rollback. The consuming workflow owns firmware policy and cleanup
when completion and ownership are known. When a completed command started
application work whose completion or ownership is uncertain, `session.abandon()`
releases host resources without issuing C4 or cleanup. Its terminal state is
`Abandoned`, and all further I/O is rejected. Abandoning cannot stop firmware
execution. Close a range reader first if it still owns the session.

For Python protocol/ownership tests, build an instrumented wheel with
`maturin build --manifest-path bindings/python/Cargo.toml --features testing`.
Install it in the test environment, then run `pytest bindings/python/tests`.
These tests replay synthetic USB exchanges through the production Rust session,
including streaming, ownership, protocol errors and disconnects. The `testing`
module is excluded from ordinary builds. No test opens a camera. Python async
and browser integration are subsequent targets.

Use `blocking.data.read_chunks(session, window, address, length)` for a streamed
E1 memory range. Construct `usb.DataWindow(base, aperture_address,
aperture_length, capacity, usb.OffsetMode.Checked)` from the selected firmware
map; use `OffsetMode.Wrapping` only where firmware's pointer addition is known
to wrap. The reader bounds commands by capacity and the host reply budget and
owns the session until iteration finishes or its context closes, just like the
memory reader. It sends no setup, retry or finalization commands. On failure,
`TransferError.partial` retains the failed command's valid bytes, including any
data delivered with the failed USB completion; previously yielded chunks remain
with the caller. The session then rejects further requests.

`session.empty_response()` sends one E8 and returns its four raw bytes; those
bytes do not indicate camera readiness or execution completion.

App80 wire types live in `fujimod.usb.app80`. `CommandCode(value)` is a distinct
immutable identifier from a selected firmware's service table. Construct
`Request(code, arguments)` with at most 32 argument bytes; input is zero padded
and never truncated. `Mailbox(page=..., offset=..., configuration_limit=...)`
requires explicit firmware values. Call
`blocking.app80.exchange(session, mailbox, request)` to stage, publish and read
one result through Rust; it returns the exact 33 mailbox bytes. It holds session
ownership for the complete operation, checks both mailbox and session limits
before sending, and does no polling or retries. The publish write awaits its
ff80 completion; mailbox bytes are not interpreted as a host wait instruction.

For raw transfer logging, create `tracing.Trace(max_events=..., max_bytes=...)`
from `fujimod.usb.tracing` and pass it as `trace=` when opening a blocking
session. `trace.drain()` returns a batch of immutable events plus
`dropped_events`/`dropped_bytes`. Events expose typed `stage`, `phase`,
`request_kind` and `failure_kind`, raw `data`, correlation IDs, endpoint and
transfer counts, and a Unix timestamp. Drain after each operation for complete
logs, including after an exception or failed session open. The recorder remains
usable while a range reader owns the session. USB work never invokes Python
callbacks. Callers choose whether trace loss stops their capture.

Emulator integration tests can use `testing.peer_session(peer, limits, ep_in=...,
ep_out=...)` instead of a fixed transcript. The peer's `control(envelope: bytes)`
and `write(data: bytes)` return `None`; `read(maximum: int)` returns bytes. These
callbacks advance the virtual endpoint model synchronously and must not reenter
the session or its range readers. Callback errors retain their original Python
exception and fence the session. All framing, validation and compositions still
run in Rust; no USB discovery or physical connection is involved. This injection
path is absent from regular builds. Production USB and trace recording never
call Python callbacks.

`blocking.data.upload(session, data, piece_bytes=..., mode=usb.DataWriteMode.Preserve)`
holds one session lock across the setup, every E0 piece and final E8. It returns
an immutable `UploadReply` with `setup`, `acknowledgement` and `bytes_written`.
Mode and piece size are required inputs. The operation sends data unchanged,
without padding, readback, retries or destination-buffer policy. A refused setup
is a structured `DeviceCommandError`; a failed exchange fences the session.
The final four bytes remain opaque and do not certify application completion.

`session.prepare_capabilities()` returns immutable `usb.CapabilitiesReply`
metadata with `byte_length` and `raw`. It does not download or parse the table.
Call `read_data(0, byte_length)` separately when the length is nonzero and within
the chosen host budget; interpret those bytes using the selected firmware.

`session.firmware_download_status()` reads firmware application state once.
`session.clear_firmware_download_status()` is a separate mutating command. Their
`FirmwareDownloadReply` exposes a `FirmwareDownloadState` enum, `failure_code`
when known, and exact `text`/`raw` fields. Writing does not trigger polling.
Clearing a status field does not stop an active firmware write.

Unclassified ff80 requests use `fujimod.usb.research`: construct
`Request(CommandCode(value), body, reply_capacity=...)`, then call
`session.research_command(request)`. The body starts after the 16-byte common
header and needs at least 16 bytes. The reply exposes typed status, primary and
secondary values, a distinct command identifier, body and raw bytes. Only the
common header is validated; the selected firmware supplies other semantics.
There is no implicit padding, polling, data transfer, retry or cleanup. The
[command catalogue](../../crates/usb/PROTOCOL.md) describes effects and limits.
