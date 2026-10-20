# Dev Tools design

## Cameras and ownership

Dev Tools holds any number of cameras at once, keyed by a stable device id. A
simulated camera is one more entry. Rust owns each camera's USB device for as
long as it is present: local tools and remote controllers are both clients of
that one owner, and nothing else opens, claims or closes the device.

Per camera:

| Field | Values |
|---|---|
| `presence` | `present`, or `gone` after unplugging. A gone camera keeps its log context and returns when the same camera is plugged in again. |
| `identity` | Unknown until first read, then vendor, product, serial and version. |
| `session` | `idle`, or `held` by a local action (named by its tool) or a remote peer. A session is short-lived: acquire, open ff80, run its operations, close, and release once the interface is fully released. |
| `sharing` | `off`, or `on` with its link and connected peers. Independent of every other field; peers are only possible session holders. |
| `uncertain` | Set when a session ends without a known completion, such as a remote vanishing mid-session. Only that camera needs reopening. |

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

## Exported files

Binary data stays binary. Descriptive fields (identity, geometry, times) may
be JSON; bytes read from a camera are stored as binary files, never as JSON
number arrays.
