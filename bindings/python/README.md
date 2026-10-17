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
`prepare_data_write`, `read_data`, `write_data` and `data_status` expose separate
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
