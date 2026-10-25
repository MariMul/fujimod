# Dev Tools design

## Cameras and ownership

Dev Tools holds any number of cameras at once, keyed by a stable device id. A
simulated camera is one more entry. Rust owns each camera's USB device for as
long as it is present: local tools and remote controllers are both clients of
that one owner, and nothing else opens, claims or closes the device.

Per camera:

| Field       | Values                                                                                                                                                                                               |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `presence`  | `present`, or `gone` after unplugging. A gone camera keeps its log context and returns when the same camera is plugged in again.                                                                     |
| `identity`  | Unknown until first read, then vendor, product, serial and version.                                                                                                                                  |
| `session`   | `idle`, or `held` by a local action (named by its tool) or a remote peer. A session is short-lived: acquire, open ff80, run its operations, close, and release once the interface is fully released. |
| `sharing`   | `off`, or `on` with its link and connected peers. Independent of every other field; peers are only possible session holders.                                                                         |
| `uncertain` | Set when a session ends without a known completion, such as a remote vanishing mid-session. Only that camera needs reopening.                                                                        |

Per page:

- `selected`: the camera the tools act on. The header lists every camera and
  switches between them.
- `log`: the single console log.

Everything else is derived. There is no page-level connected, sharing or
blocked state.

- A tool or Console command is enabled when the selected camera is present,
  idle and not uncertain.
- While a remote holds the session, tools show who is using the camera. While a
  local tool holds it, that tool shows it is running.
- A share link names one camera. Stopping a share removes its peers and changes
  nothing else.

## Log

There is one log, the console. Every entry carries its camera and session
holder, and filters by layer and by camera.

- The transport logs; tools do not. Each ff80 exchange is one FF80 entry
  carrying its meaning (operation, address, length), with the raw bytes in its
  expandable detail.
- The Connection layer holds connection events only (camera present or gone,
  sharing started or stopped, errors) and filters like every other layer.

## Firmware identity

Camera details resolve the service internal version through the catalog's
firmware version map and display the public release when known. An internal
version below that product's highest known version gets an official update
link. Unmapped lower versions say they appear outdated; newer or unparseable
versions do not get that inference. Simulated cameras display their version
without a hardware update prompt.

## Exported files

Binary data stays binary. Descriptive fields (identity, geometry, times) may
be JSON; bytes read from a camera are stored as binary files, never as JSON
number arrays.

The browser entry point is `pageCameraModel()` in `usb-camera-model.ts`. It pins
one model to the browser page, outside component ownership, so component and
module hot swaps keep every Rust device owner and session alive. Server renders
create independent models. The model owns connection, peer and FF80 log entries;
Rust supplies decoded traffic, including console command and output bytes, tagged
with the device and session holder. Empty, incomplete console polls wait 50 ms
before returning so callers can poll without a busy loop.

Development and local-preview pages start `startDevShare(model, options)` once.
Each present USB camera gets its own share and registry WebSocket through
`model.setSharing`; remote cameras are never registered as local hardware.
Identity, presence and battery status update that camera's registration. Stopping
a share suspends automatic sharing for that camera until it reconnects. A shared
camera keeps its offline registration while unplugged. Page unload withdraws all
registrations synchronously. These attachments never change device connectivity
or take the local session lock.

## Workbench layout

Until a camera is known, Dev Tools shows the start screen: how to enter service
mode, Connect camera, and Simulated camera, which lists every camera saved in the
emulator and starts the chosen one as a simulated camera over simulated USB. It
stays unavailable until the build can run one, and offers the emulator when no
camera is saved. Once a camera is known:

- A camera strip lists every camera with its state and whether it is shared,
  plus Connect camera.
- The selected camera's details, at the top of the tools column, show its identity (read automatically when it
  appears), its state, the service-mode caution, and its sharing switch.
- Tools start with the numbered path in `GETTING-STARTED.md` (back up flash,
  dump camera state, enable debug mode, restart, states to capture), followed
  by the other tools.
- The console log fills the right side. Its camera column and camera filters
  appear only once more than one camera is known.

Controls are neutral and compact; green marks state and the one primary
action, amber marks writes.
