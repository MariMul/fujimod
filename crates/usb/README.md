# FujiMod USB

Async Rust access to Fujifilm's reverse-engineered ff80 service/debug interface.
This first implementation provides nusb discovery and explicit interface
selection, C0/C2/C4 session lifecycle, D0/D1 command exchanges, information
selectors, configuration commands, debug events and console polling, log reads,
diagnostic queries, E0/E1/E8 data transfers and composed memory and diagnostic-data range operations.

`ff80/commands/` encodes one firmware request at a time. `operations/` composes
commands. nusb handles platform USB access; its adapter has a private test seam.
Body/firmware definitions supply memory limits explicitly. The session's
advertised packet size, USB endpoint packet size and the host reply budget
are separate quantities. The library does not enable debug access or change
service mode to make a command work.

```rust,no_run
use std::{num::NonZeroU32, time::Duration};
use fujimod_usb::{Result, connection::{Connection, InterfaceSelection, TransferOptions},
    ff80::commands::info::InfoSelector};

async fn identify(selected: &nusb::DeviceInfo, interface: InterfaceSelection) -> Result<String> {
    let connection = Connection::open(selected, interface, TransferOptions {
        timeout: Duration::from_secs(5),
        max_reply_bytes: NonZeroU32::new(1024).unwrap(),
    }).await?;
    let mut session = connection.begin_session().await?;
    let product = session.device_info(InfoSelector::Product).await?;
    session.close().await?;
    Ok(product.text)
}
```

The caller chooses the device and configuration/interface/alternate setting.
Opening configures that selected USB interface if necessary, without detaching
kernel drivers or sending camera reset requests. Descriptor inspection requires
exactly one bulk endpoint in each direction; ambiguous layouts are rejected.
Native enumeration and claiming use nusb's `smol` support for blocking OS work;
USB transfers remain asynchronous. No particular executor is required.

Cancellation, timeout, transfer failure or malformed replies leave the session
in `CompletionUnknown`. Further exchanges, including close, are rejected.
Dropping releases host resources; it does not promise that camera execution
stopped. A validated camera error is returned as `DeviceStatus`, including both
raw status values, rather than interpreted as a request to wait or retry.
This conservative initial core has no recovery/reopen automation.

Use `ReadMemory` and `WriteMemory` for individual requests. Their constructors
validate the explicit firmware limits and the 32-bit address extent. Use
`operations::memory::MemoryReader` for a range: each `next_chunk().await` issues
one request and returns one buffer. It bounds each command by firmware limits,
the negotiated packet limit and the host reply budget, and retains the number
of successfully returned bytes. Previously returned buffers remain with the
caller if a later chunk fails.

Use `operations::data::DataReader` to download a CPU address range through E1.
Supply a `DataWindow` with the diagnostic pointer, allowed address aperture,
per-command capacity and `OffsetMode::Checked` or `OffsetMode::Wrapping` from
the selected firmware's definition. Each pull issues one command bounded by
capacity and host reply budget; the negotiated debug packet size does not limit
bulk data commands. Querying and validating the diagnostic pointer remains a
separate caller policy. No setup command, retry or E8 is sent implicitly.
`PartialRead.data` includes valid fragments from the failed USB completion as
well as earlier fragments in that command. `bytes_completed()` counts only
chunks already returned successfully.

`Session::empty_response()` exposes E8's four raw acknowledgement bytes. It is
not a readiness poll or a wait-for-execution API.

The initial implementation is tested on native macOS with synthetic exchanges;
no hardware access is part of the test suite. Browser builds are a subsequent
capability. Native Python bindings live in `bindings/python/` at the workspace
root. All public I/O is async and the internal test interface does not
require `Send` futures. Browser execution is not yet validated.

Protocol references are `fftlib.py` and `ffjlib.py` from
[tiredboffin/fffw](https://github.com/tiredboffin/fffw/tree/bedc091e0b54a1a34aaf6929dd08e1db36d13b08/ff80), and
X70 View's existing ff80 callers. The upstream MIT notice is retained in
[LICENSE.fffw](LICENSE.fffw). No firmware images or private camera data are
included.

Run `cargo test --workspace --all-features` from the repository root.

The opt-in `testing` feature supplies strict synthetic exchange scripts through
the same session implementation. It does not enumerate or open USB devices.
Python's test build uses it to verify streaming and ownership across the language
boundary; regular builds contain only the nusb backend.

App80 has a separate [`app80`](src/app80.rs) wire codec: `CommandCode`, bounded
`Request` and exact `Reply`. Identifiers and argument meanings come from the
selected firmware's service table; the codec does not equate command bytes,
ff80 status words or firmware callback dispositions.

`operations::app80::Mailbox` composes the ff80 carrier. Supply the selected
firmware's page, command-byte offset and configuration limit. `exchange()`
stages 32 argument bytes, publishes the 33-byte request, awaits that write's
ff80 reply, then reads the mailbox once. The complete operation's host budget
is checked before the first write. Firmware asynchronous completion is handled
by the ff80 exchange; there is no invented mailbox busy bit, automatic poll,
retry, reset or cleanup. A failed or cancelled USB exchange fences the session.
Command-specific result decoding and named body APIs belong above this carrier.
