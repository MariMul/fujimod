# FujiMod USB

Async Rust access to Fujifilm's reverse-engineered ff80 service/debug interface.
This first implementation provides nusb discovery and explicit interface
selection, C0/C2/C4 session lifecycle, D0/D1 command exchanges, information
selectors, configuration commands, debug events and console polling, log reads,
diagnostic queries, capabilities metadata, E0/E1/E8 data transfers and composed memory and diagnostic-data range operations.

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
    let mut session = connection.begin_session(None).await?;
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

`operations::data::upload` owns the complete setup/E0/E8 sequence. Supply the
setup mode and E0 piece size explicitly. It checks the final receive budget
before setup, stops on a framed setup error, and sends each piece and E8 once.
Its `UploadReply` preserves the raw setup fields and four acknowledgement bytes.
Padding, destination-buffer ownership and readback/retry policy stay above this
composition. The debug memory packet limit does not constrain E0 data pieces.

`Session::empty_response()` exposes E8's four raw acknowledgement bytes. It is
not a readiness poll or a wait-for-execution API.

The core is tested on native macOS and as Wasm in a JavaScript runtime using
synthetic exchanges. nusb's WebUSB backend compiles for `wasm32-unknown-unknown`.
The [JavaScript bindings](../../bindings/javascript/README.md) expose that
backend; physical browser/camera integration remains unvalidated. No hardware
access is part of the test suite. Native Python bindings live
in `bindings/python/` at the workspace root, with an awaitable API and an explicit
blocking adapter. All Rust public I/O is async and the
internal test interface does not require `Send` futures.

Protocol references are `fftlib.py` and `ffjlib.py` from
[tiredboffin/fffw](https://github.com/tiredboffin/fffw/tree/bedc091e0b54a1a34aaf6929dd08e1db36d13b08/ff80), and
X70 View's existing ff80 callers. The upstream MIT notice is retained in
[LICENSE.fffw](LICENSE.fffw). No firmware images or private camera data are
included.

Run `cargo test --workspace --all-features` from the repository root.

The opt-in `testing` feature supplies strict synthetic exchange scripts through
the same session implementation. `testing::Peer` supplies synchronous virtual
endpoint completions for firmware emulators using this same Rust core; it exists
only in testing builds. These paths do not enumerate or open USB devices.
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

`trace::Trace` optionally records the ff80 USB transfers beginning with session
open. Pass a clone to `Connection::begin_session(Some(trace.clone()))` and drain
batches independently of the session. Records retain timestamps, correlation
IDs, endpoint addresses, submitted control/OUT bytes, received IN bytes and
partial failure data. Host future cancellation is recorded without implying
that the camera stopped executing. A USB completion does not certify a valid
firmware reply; protocol decoding still determines session state.

The queue bounds both event count and retained payload bytes. It discards old
records (or an oversized record whole) and reports exact loss counts with each
batch; it never truncates payloads silently or changes USB outcomes. Recording
calls no user code. File writing, trace-loss policy and GUI presentation belong
to callers. With tracing disabled, transfer payloads are not copied for logging.

`Session::prepare_capabilities()` sends one `0x580000` command and returns
`CapabilitiesReply { byte_length, raw }`. A later E1 `read_data` retrieves the
table; that read remains a separate action. Length comes from the firmware and
is not fixed to one body's table size. The library preserves metadata fields
without treating the table as host timeouts or inferring firmware-specific
record meanings. A nonzero device status is a completed `DeviceStatus` error;
a malformed header fences the session.

The [service command catalogue](PROTOCOL.md) lists direct APIs, their effects,
and unclassified operations. `firmware_download_status()` reads state once;
`clear_firmware_download_status()` explicitly mutates it. Both retain text and
raw bytes alongside a `FirmwareDownloadState` enum. Writing is application
state, never an implicit instruction to poll, wait or retry.

Use `ff80::commands::research::{CommandCode, Request}` and
`Session::research_command` for unclassified requests. The explicit reply
capacity is checked before sending. The result validates the common header,
preserves raw status/body bytes and performs no command-specific interpretation.
It is distinct from App80's one-byte identifier and mailbox codec.

## Wasm core checks

The workspace's `.cargo/config.toml` enables the WebUSB APIs required by nusb.
Install the Wasm standard library and a wasm-bindgen test runner matching the
`wasm-bindgen` version in `Cargo.lock` (currently `0.2.129`):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root target/tooling
PATH="$PWD/target/tooling/bin:$PATH" cargo test -p fujimod-usb \
  --target wasm32-unknown-unknown --features testing --lib --test wasm_core
```

Run from the workspace root with Node on `PATH`, or use `mise run rust-wasm-test`
after installing the runner. The version match is required by wasm-bindgen's
binary format. These tests execute lifecycle, tracing, cancellation and timeout
code as Wasm without device discovery. They do not claim browser permission or
physical USB validation.

Control and bulk transfers share a host deadline. This covers WebUSB, where
nusb cannot apply a control-transfer timeout itself. On expiration the session
is fenced; the camera may still be processing the transfer. Trace timestamps
use the JavaScript wall clock on Wasm and preserve the same epoch/type as native
builds, without calling the unsupported Wasm standard-library wall clock.

Browser bindings expose asynchronous sessions, owned chunk iterators, structured
errors, explicit abandonment and pull-based tracing through one `usb` namespace.
They reside in `bindings/javascript/`, separately from the implementation crate
and `apps/web`. See their README for the static-host build and mock-WebUSB tests.

## Firmware-qualified App80 controls

`fujimod::defs::service::Definition` loads the selected firmware's service data.
`app80::catalogue::Catalogue` resolves its ordered mode rules and typed codecs;
`operations::app80::Service` binds those codecs to that definition's ff80
mailbox. The resulting `Invocation` keeps its request and carrier together.
Preparation does no I/O and never changes the camera's service mode.

```rust,ignore
let identity = FirmwareIdentity::new(firmware_id, firmware_sha256)?;
let definition = Definition::from_json(&definition_json, &identity)?;
let service = Service::new(definition)?;
let request = service.prepare(
    ServiceModes::new(current_service_flags)?,
    Command::ReadPins { bank: PinBank::new(0)? },
)?;
let reply = request.execute(&mut session).await?;
```

These internal service flags are distinct from camera UI or shooting modes.
Missing, ambiguous, disabled or unreviewed entries fail selection. Command
identifiers, slot positions and mailbox locations come from data, with no
fallback to another body. Typed inputs cover pin reads/writes, channel access,
profile-byte bank reads/writes/reset, download status, configuration copy/save,
and service word access. Remaining entries retain explicit research envelopes.
These are wire codecs; a firmware handler may ignore lower-level hardware
errors, and `NoPayload` does not certify a physical effect.

Typed results preserve the raw mailbox alongside `ReplyValue`. A save result
of 1 is `SaveStatus::Failed`, never a host wait instruction; unknown result
values remain `Unknown`. The save output starts at an unknown sentinel. Word
read/write results distinguish values, firmware rejection and unknown status.
The reviewed word codec encodes big-endian address/value bytes and checks its
known alignment/address windows before I/O. Research replies stay opaque.
A wrong command echo in a typed nested reply fences the session, preventing
further requests or implicit close. No polling, retry or cleanup is added.

Run the read-only catalogue inspector with an explicit identity:

```sh
cargo run -p fujimod-usb --example service_catalogue -- definition.json firmware-id SHA256
```

It prints metadata and performs no USB discovery. Native/Pyodide/JavaScript
bindings for these catalogue controls are the next integration step; the
existing explicit App80 wire and mailbox bindings remain available.
