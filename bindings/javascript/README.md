# FujiMod JavaScript bindings

This package provides `import { initialize, usb } from 'fujimod'` for browser
service/debug tools. Its Rust binding crate belongs to the root Cargo workspace;
commands, framing, range operations and transfer logging run in `crates/usb`.
There is one JavaScript package and one `usb` namespace. Firmware/body maps and
application policies are supplied by callers.

## Build and test

Use the repository's Rust and Node toolchains (`mise install`). Install the
Wasm standard library and the wasm-bindgen CLI matching `Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root target/tooling
cd bindings/javascript
npm ci
npm run build
npm test
```

`WASM_BINDGEN` can select an explicitly installed CLI executable. By default,
the build script uses `target/tooling/bin/wasm-bindgen` from the repository root.
`pkg/`, `dist/` and `node_modules/` are generated and ignored. The package remains
private while APIs and release metadata are being prepared; these commands do
not publish it or change `apps/web`.

Tests run compiled Wasm in Node. Instrumented tests exercise the real protocol
core with synthetic endpoints. Production tests exclude that instrumentation
and exercise nusb's WebUSB backend against a fake `navigator.usb` device,
including descriptors, interface/alternate selection, endpoint numbers, data
transfers, picker cancellation and a stalled control transfer. No test opens
hardware. An additional headless Chromium check exercises the production package
in a real browser against that same fake USB API:

```sh
npx playwright install chromium
npm run test:browser
```

`PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` can select an explicitly installed Chromium
binary. Permission UI and physical camera integration remain unvalidated.

## Connect and identify

Call `await initialize()` once before enabling the GUI's connect action. It
loads the adjacent Wasm asset. Host the package/assets over HTTPS, such as
GitHub Pages, or localhost, in a browser supporting WebUSB. From the user's
connect action, call `usb.requestDevice(usb.DeviceFilter.Diagnostic)` to open the
browser picker. Cancellation returns `undefined`. `usb.discover(filter)` lists
already granted devices without opening a picker.

Inspect `device.interfaces()` and explicitly choose a configuration, interface
and alternate setting. Select limits from the body's firmware definition. The
library has no default camera, interface or firmware map.

```ts
import { initialize, usb, type Device } from 'fujimod';
await initialize();

async function identify(
  device: Device,
  selected: InstanceType<typeof usb.InterfaceSelection>,
  limits: InstanceType<typeof usb.FirmwareLimits>,
) {
  const options = new usb.TransferOptions(5000, 1024 * 1024);
  const session = await usb.Session.open(device, selected, limits, options);
  try {
    const reply = await session.deviceInfo(usb.InfoSelector.Product);
    return reply.text;
  } finally {
    if (session.state === usb.SessionState.Ready) await session.close();
    else session.abandon();
  }
}
```

`TransferOptions` specifies a per-transfer timeout in milliseconds and a host
reply budget. `FirmwareLimits` requires memory-read, memory-write, configuration
and console limits, in that order. Addresses, sizes and raw protocol fields are
checked integers; strings, fractions, negative values, overflow and nonfinite
numbers are rejected. Choices use generated TypeScript enums, with runtime
validation of their number/type/domain. Data arguments require `Uint8Array`;
bytes are copied across the Wasm boundary rather than exposing a borrowed view.

## Commands and composed operations

Session methods such as `readMemory`, `writeMemory`, `readConfiguration`,
`diagnosticQuery`, `submitConsole` and `pollConsole` each perform one command.
`configurationGeometry`, `prepareCapabilities` and the two firmware-download
status methods retain typed metadata and raw replies. `readLog` is consuming
retrieval: firmware can advance its log cursor or clear its overrun state.
`clearFirmwareDownloadStatus` is an explicit mutation, separate from reading
state or identifying the camera. Application states such as Writing or console
Pending never cause implicit waits, retries or cleanup.

Higher-level operations live in their own groups:

- `usb.memory.readRange` / `writeRange` compose bounded memory commands.
- `usb.memory.readChunks(session, address, length)` returns an async iterator
  with `bytesCompleted` and an explicit `close()`.
- `usb.data.readChunks(session, window, address, length)` streams E1 ranges;
  `DataWindow` requires an explicit base, aperture, capacity and offset mode.
- `usb.data.upload(session, bytes, pieceBytes, mode)` owns setup, E0 pieces and
  E8 as one operation, preserving raw setup fields and final acknowledgement.
- `usb.app80` supplies distinct one-byte command IDs, requests and an explicit
  mailbox carrier. Codes and result meanings come from the selected firmware.
- `usb.research` supplies distinct ff80 command IDs and unclassified request
  bodies with an explicit reply capacity. Only the common header is validated.

```ts
const chunks = usb.memory.readChunks(session, address, length);
try {
  for await (const bytes of chunks) {
    await sink.write(bytes);
  }
} finally {
  chunks.close();
}
```

A range owns its session between pulls and never reads ahead. Finishing, closing
or breaking a `for await` loop releases that ownership without sending C4.
Completed chunks stay with the caller on failure. A failed bulk command's valid
partial data remains in `error.partial`. A currently pending pull must finish
before `close()`; explicitly abandon its session to stop host processing.

The [command catalogue](../../crates/usb/PROTOCOL.md) defines effects and the
boundary between direct mechanisms, nested protocols and compositions. Debug
activation, safe exit, callbacks, allocation, persistent settings and mod
installation belong in firmware-qualified application code.

## Ownership, cancellation and errors

Each session operation reserves its session before its Promise is returned. Overlapping
commands reject with `ErrorKind.Busy`; a range reports `SessionState.ReadingRange`
until it releases ownership. Timeout, transfer failure and malformed replies
leave `CompletionUnknown`. Further commands cannot consume an unfinished reply.

`close()` consumes the session on success or failure. `Closed` requires a
received two-byte C4 reply; those bytes remain opaque. A failed close leaves
`CompletionUnknown` and cannot be retried. It does not certify camera shutdown.

JavaScript Promises have no implicit cancellation. `session.abandon()` explicitly
releases idle host resources, cancels a pending host operation or range pull, and
makes the session terminal (`Abandoned`). It sends no further USB command or
C4. The browser may already have submitted a transfer, and camera execution may
continue. Dropping or ignoring a Promise alone does not cancel its operation.

Errors are ordinary `Error` objects with a typed `kind`. Use `usb.isError(error)`
to narrow a TypeScript `unknown` to `UsbError`. Transfer errors retain stage,
request kind when applicable, backend failure kind/code, counts and partial
bytes. Protocol errors retain a `ProtocolFailureKind` and available numeric
fields. Device errors retain command/primary/secondary values. Research replies
and diagnostic results retain a `CommandStatus` enum alongside their raw fields.
No integer is interpreted as an instruction to halt or wait.

## Raw logging

Create `new usb.tracing.Trace(maxEvents, maxBytes)` and call
`options.setTrace(trace)` before opening a session. `trace.drain()` works
independently of session ownership, including during a pending command and
after opening fails. Each immutable event retains typed stage/phase/request
kind, raw bytes, endpoint and counts. Correlation/sequence IDs and exact loss
counts are JavaScript `bigint`; timestamps are Unix milliseconds.

The recorder bounds event count and retained payload bytes. It reports loss
without truncating payloads silently or changing USB outcomes. Consumers choose
when to drain and whether losing trace data should stop their application.

Python in Pyodide can use this same initialized module. Call
`registerPyodide(pyodide)` and install the browser wheel from
[`bindings/python`](../python/PYODIDE.md). `session.cancelPending()` and chunk
iterators' `cancelPending()` cancel host work without abandoning the session.
Await the pending Promise before reusing ownership; the session is fenced if
an exchange was incomplete. Cancelling before dispatch leaves it ready. Host
cancellation rejects with `ErrorKind.Cancelled`; it never stops camera code.
`abandon()` remains the distinct terminal action.

## Firmware-selected service controls

`defs` loads the shared firmware data. `usb.app80` exposes typed command factories
and executes their invocations through the same Rust codecs as native Python:

```ts
import { defs, usb } from 'fujimod';

const identity = new defs.FirmwareIdentity(firmwareId, firmwareSha256);
const definition = defs.service.Definition.fromJson(definitionJson, identity);
const service = new usb.app80.Service(definition);
const invocation = service.prepare(
  new defs.service.ServiceModes(currentServiceFlags),
  usb.app80.Command.readPins(new usb.app80.PinBank(0)),
);
const reply = await usb.app80.execute(session, invocation);
const value = reply.value;
if (value.kind === usb.app80.ReplyKind.Pins) {
  console.log(value.levels);
}
```

The identity qualifies the loaded data; the caller still identifies the camera
and checks execution eligibility. Preparation performs no USB I/O or mode change.
Command codes, argument positions, mode rules and mailbox come from the selected
definition. An invocation keeps the request and mailbox together. Commands and
profile actions have named factories rather than raw selector constructors.

Definitions preserve ordered entries, duplicate codes, mode availability and
transitions. Handler and parameter-table addresses use `bigint`. Metadata and
reply values use immutable unions tagged by enums; TypeScript narrows payload
fields using their `kind`. Raw mailbox getters return independent `Uint8Array`
copies. `invocation.decode(raw)` supports offline decoding without a session.

`defs.isError(error)` exposes a typed `DefinitionFailure` in `error.reason`;
`usb.app80.isError(error)` similarly exposes `CommandFailure`. Unsupported,
ambiguous, disabled or unreviewed controls do not fall back to another body.
Research remains explicit through `service.prepareResearch(code, modes, bytes)`.

Save failure and unknown status values remain data. Word results distinguish a
returned value, rejection, confirmation and unknown status. A malformed typed
reply fences the session; an interrupted publication follows normal cancellation
and ownership rules. There is no implicit poll, retry or close. The Node and
Chromium production tests execute this path through nusb and fake WebUSB.

Peripheral factories include `Command.registerControl(RegisterAction.powerWrite(...))`,
`Command.i2c(I2cAction.read(I2cPort.Port0, command, new I2cLength(length)))`,
`Command.accumulator(AccumulatorAction.StopAndRead)` and `Command.peripheralReport()`
under `usb.app80`. `new I2cPayload(bytes)` owns a checked 1–10 byte copy; the
write-only route uses `I2cAction.writeSingle(command, value)`. Result variants
carry copied bytes or an immutable tuple of three low counter halfwords.

Media factories add `Command.displayOption`, `displayMode`, `sound`, `hdmi`,
`parameterService`, `chargeControl` and `chargeStatus`. Inputs use the corresponding
action factories, `Toggle`, `DisplayMode`, sound enums and `ChargeQuery`.
`SoundClip`, `HdmiPacket` and `HdmiValue` validate the clip index, exact packet
length and 12-bit values in Rust. Charge reply unions use different status enums
for applying control, presence and reported register-state groups; unknown bytes
retain `rawStatus`. No charge value starts an implicit poll.

`Command.audioCapture(new AudioCount(n), AudioRoute.Request82)` checks 0–10
firmware chunks; `Command.audioPlayback(AudioPlayback.AllLanes)` can instead
zero either interleaved lane. Samples stay in camera buffers; USB transfer is a
separate operation. Results use `AudioResultKind`. `Command.eraseStorage` takes
`StorageErase` and returns `StorageEraseResultKind`. These selectors can clear
persistent regions and profiles. `Command.storageStatus()` returns
`StorageStatusKind`: zero is deliberately ambiguous, and other classifications
retain their numeric names. `Command.formatStorage()` formats the selected
drive, and `Command.displaySetup()` invokes the internal display setup sequence.
These last two have no payload result. No result starts host polling or cleanup.

`Command.nandReadBlocks(NandSpare.Include, group)` reads eight firmware NAND
blocks into its RAM buffer without downloading them. The reply uses
`NandReadResultKind`; output 1 is an application result, never a host wait.
`Command.option41()` returns `Option41StateKind.Zero`/`NonZero`/`Unknown` for the
firmware getter, without asserting drive presence. `Command.wlanControl(Toggle)`
requests a control-line action whose enable path also increments a counter.

`RadioAction.primary(mode, channel, power, option)` uses `RadioMode`, checked
`RadioChannel` (0–14), checked `RadioPower` (encoded 0–127) and `Toggle`.
`secondary(mode, channel)` excludes the unused fields. Pass either to
`Command.radioControl`. `Command.signalStrength(new PeerAddress(bytes))` owns
six bytes and returns opaque `rawValue`, which can include unchanged or
uninitialized firmware output. `Command.iperf()` uses stored configuration text
and returns opaque `rawResult`; firmware service acknowledgement can precede
running the test. Neither result starts a host poll or certifies a measurement.

Service enums, checked quantities, command/action factories, structured replies
and public registrations come from the [shared Rust generator](../generator/README.md).
Builds regenerate them automatically; validation remains in the core.
