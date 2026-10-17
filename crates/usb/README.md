# FujiMod USB

Async Rust access to Fujifilm's reverse-engineered ff80 service/debug interface.
This first implementation provides nusb discovery and explicit interface
selection, C0/C2/C4 session lifecycle, D0/D1 command exchanges, information
selectors, direct memory reads/writes and a pull-based range reader.

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

The initial implementation is tested on native macOS with synthetic exchanges;
no hardware access is part of the test suite. App80, configuration commands,
bulk diagnostic strategies, Python bindings and browser builds are subsequent
capabilities. All public I/O is async and the internal test interface does not
require `Send` futures. Browser execution is not yet validated.

Protocol references are `fftlib.py` and `ffjlib.py` from
[tiredboffin/fffw](https://github.com/tiredboffin/fffw/tree/bedc091e0b54a1a34aaf6929dd08e1db36d13b08/ff80), and
X70 View's existing ff80 callers. The upstream MIT notice is retained in
[LICENSE.fffw](LICENSE.fffw). No firmware images or private camera data are
included.

Run `cargo test --workspace --all-features` from the repository root.
