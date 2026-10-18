# Camera bug reports

`/bug#?report=<base64url(JSON)>` is a Solid entry point. The fragment is decoded
in the browser; it is never sent as part of an HTTP request. `link.schema.json`
is also the source for the camera's committed, generated C codec. Inline
`archive` bytes are base64url TAR data. Otherwise the page retrieves the named
TAR over the existing diagnostic USB memory protocol, or accepts a local file.
All paths validate the complete byte length, CRC32 and TAR headers before
reporting. The two acquisition paths remain visible side by side, including
platform-specific SD-card directions. The actual TAR inventory replaces the
initial data categories after capture. Nothing is sent automatically.

**Upload this information** sends the archive and description to the sharing
Worker's `/issue-reports` endpoint. The archive stays private in R2; a server-side
token creates the public GitHub issue without requiring a reporter account.
The page explains that the description is public. Provisioning, request limits
and reconciliation are documented in `apps/usb-sharing/ISSUE_REPORTS.md`.
**Download a copy** remains available independently (local gzip/ZIP).

**Delete from camera after uploading** is checked by default and enabled only
for a live USB-acquired report. A matching durable storage acknowledgement must
arrive before deletion. Failed uploads retain the file; repeated upload attempts
reuse the same idempotency key. Pending GitHub creation still has a durable
archive receipt and report ID.

**Open Dev Tools** switches to the existing Solid component in place, handing
it the selected device and live session. Its opt-in `connectOnMount` starts
that adapter; it does not open another device chooser or USB session.

## X70 1.10 export channel, version 1

The URL identifies a 32-byte-aligned channel inside the checked mod region
`0x40390000..0x40600000`. The browser verifies the complete fixed header, ABI
magic and build tag before writing. It never sends console commands to download
reports, calls stock functions by address, changes modes or modifies settings.
The camera validates filenames and constructs the path in `FUJIMOD/REPORTS`.
The channel is initialized at boot, so a QR can retrieve a saved report after a
restart of the same build. JIG/service mode is refused. The camera temporarily
enables diagnostic USB in RAM while its report screen is open. Reconnect a
cable that was already attached. The prior flag is restored after dismissal
and disconnect, keeping an established Dev Tools session available. After a
camera restart, reopen the report screen or enable USB DEBUGGING before connecting.

| Offset | Little-endian 32-bit field                                               |
| ------ | ------------------------------------------------------------------------ |
| 0      | Magic `0x31425246`                                                       |
| 4      | Version, 1                                                               |
| 8      | Selected capture nonce                                                   |
| 12     | TAR byte length                                                          |
| 16     | Complete TAR CRC32                                                       |
| 20     | Host command (worker consumes it to zero)                                |
| 24     | Status: 1 busy, 0 complete, negative or other positive value is an error |
| 28     | Current returned chunk offset                                            |
| 32     | Returned chunk byte length                                               |
| 36     | Current stock UI mode; 12 is refused                                     |
| 40     | First eight hexadecimal digits of the build commit                       |
| 44     | Capabilities: bit 0 permits deletion of a verified selected report |
| 48–60  | Reserved                                                                 |
| 64     | 16 KiB data buffer                                                       |

Select (command 4) takes a 32-byte request in the data buffer: NUL-terminated
`Rxxxxxxx.TAR` in bytes 0–15, expected length at 16, CRC32 at 20, nonzero nonce
at 24, reserved word at 28. Command 1 reads the next chunk; 2 resets the stream;
3 closes it. Command 5 deletes the selected report only after a complete,
CRC-verified read and close, then clears its nonce. New selection, capture or
errors clear verification. Older builds have no delete capability. Only the command word and selection payload are host-writable.
The existing scanner wakes the sleeping report worker, which owns all stock
file-service operations and persistent request packets. No new periodic timer
or background USB poll is installed on the camera. Client polling exists only
while a requested transfer is in progress.

Tests cover inline and USB acquisition, malformed hashes, wrong builds and
modes, chunk sequencing, corruption, local download compression, explicit
delivery, and the live-session handoff. Camera-side tests separately execute
the ARM implementation and stock filesystem; these do not qualify physical USB
hardware timing.

## Browser integration test

Pass the output directory from the successful camera walkthrough. This uses
its actual QR URL and stock-written TAR, checks phone/desktop layout, opens the
file locally, and compares the downloaded archive byte-for-byte. Upload is
intercepted and its exact TAR digest is checked. No outgoing write is allowed
before the explicit upload click; no real issue is submitted. Set `FUJIMOD_BROWSER=webkit` for the Safari engine.

```sh
FUJIMOD_WEB_URL=https://your-host.ts.net/ node src/bug/browser.mjs PATH_TO_GUIDE
```

Screenshots and results are written to ignored `apps/web/build/bug-report/`.
