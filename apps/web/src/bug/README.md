# Camera bug reports

`/bug#?report=<base64url(JSON)>` is a Solid entry point. The fragment is decoded
in the browser; it is never sent as part of an HTTP request. `link.schema.json`
is also the source for the camera's committed, generated C codec. Inline
`archive` bytes are base64url TAR data. Otherwise the page retrieves the named
TAR over the existing diagnostic USB memory protocol, or accepts a local file.
All paths validate the complete byte length, CRC32 and TAR headers before
reporting. There is no upload endpoint. Only the explicit **Download and
report** action downloads a file and opens a GitHub issue with the description.
The TAR is gzip-compressed locally for GitHub attachment compatibility (or
wrapped in a store-only ZIP when CompressionStream is unavailable).

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
restart of the same build. JIG/service mode is refused.

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
| 44–60  | Reserved                                                                 |
| 64     | 16 KiB data buffer                                                       |

Select (command 4) takes a 32-byte request in the data buffer: NUL-terminated
`Rxxxxxxx.TAR` in bytes 0–15, expected length at 16, CRC32 at 20, nonzero nonce
at 24, reserved word at 28. Command 1 reads the next chunk; 2 resets the stream;
3 closes it. Only the command word and selection payload are host-writable.
The existing scanner wakes the sleeping report worker, which owns all stock
file-service operations and persistent request packets. No new periodic timer
or background USB poll is installed on the camera. Client polling exists only
while a requested transfer is in progress.

Tests cover inline and USB acquisition, malformed hashes, wrong builds and
modes, chunk sequencing, corruption, local download compression, explicit
delivery, and the live-session handoff. Camera-side tests separately execute
the ARM implementation and stock filesystem; these do not qualify physical USB
hardware timing.
