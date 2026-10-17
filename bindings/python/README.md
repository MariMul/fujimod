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
I/O is rejected. Releasing Python objects does not send a camera reset, mode
transition or rollback. The consuming workflow owns firmware policy and cleanup
when completion and ownership are known.

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
