# Share links and direct WebRTC

Dev Tools' **Share** button lists the connected camera. Selecting it creates a
capability URL with `#?remotecontrol=<ticket>`. Touch devices invoke the native
share sheet; desktop browsers copy the URL and show a toast. The popup displays:

**Anyone with a share link will be able to control your USB device.**

Opening the URL loads Dev Tools, selects **Shared camera**, discovers the single
exported device and opens its selected interface automatically. It does not
open a USB picker or fall back to a local device. The owner records **Remote
controller connected** and **Remote controller disconnected** in the connection
log. Discovery connections can be brief; opening a protocol session establishes
its own connection.

## Native tools

The complete browser URL is also the connection setting for any native FujiMod
Rust or Python tool using normal discovery:

```sh
export FUJIMOD_USB_PROXY_URL='https://your-site/dev-tools/#?remotecontrol=...'
# Run the intended FujiMod command normally. No separate token is required.
```

Explicit Rust/Python/JavaScript/Pyodide discovery accepts the same URL:
`discover_proxy(link, "", filter)` / `usb.discoverProxy(link, "", filter)`.
The ordinary Connection and Session APIs execute their existing codecs and
tracing through the selected transport. No firmware command is implemented by
the rendezvous service or WebRTC layer.

## Connection path

WebRTC carries USB requests and completions on a reliable ordered data channel.
The native implementation uses [str0m](https://docs.rs/str0m/); Wasm uses the
browser's RTCPeerConnection through the same crate's bundled JavaScript module.
A bounded framing layer splits large JSON/base64 transfers into 16,000-byte
chunks. Transfer capacities and timeout fencing remain the USB proxy's existing
8 MiB and 120-second limits.

WebSockets provide rendezvous, not NAT traversal. The signaling endpoint is
`/usb-proxy/signal` on the Rust development service or the production
[Cloudflare Worker](../../apps/usb-sharing/README.md). It routes encrypted SDP between
an ephemeral owner room and its clients. It never receives USB requests or
payloads. A room disappears when its owner disconnects; links are not persistent
camera identities. Stop sharing or close the owner's page to revoke the link.

Production remains static JavaScript on GitHub Pages. The Worker provides
rendezvous and temporary TURN credentials independently of the static host;
Cloudflare DNS is not required. Set the GitHub repository variable
`FUJIMOD_SIGNAL_URL` to the deployed Worker WebSocket endpoint before releasing
the Pages app. An unconfigured production build disables Share.

For the local Vite development and preview servers, `/usb-proxy/` forwards to
the local Rust service. Static deployments must set `FUJIMOD_SIGNAL_URL` at
build time to a reachable `wss://.../usb-proxy/signal` endpoint. The private
workspace's installed service and named Tailscale URL are documented in its
`tools/usb_proxy/README.md`. Viewers must be able to reach the signaling endpoint;
a link containing a private tailnet endpoint is limited to that tailnet.

Tickets use independent 256-bit random room IDs and capability keys. The key
stays in the fragment, outside ordinary HTTP requests and access logs. SDP is
AES-256-GCM encrypted with a fresh 96-bit nonce and room/client-bound additional
data. The data channel's DTLS connection is negotiated through that authenticated
exchange. The rendezvous server cannot substitute a peer certificate or read
USB traffic. Possessing the URL authorizes control of the selected device.

The default tickets include Cloudflare's
[public STUN service](https://developers.cloudflare.com/realtime/sfu/get-started/connection-patterns/).
STUN discovers mapped addresses; it does not relay USB data. A configured
production Worker adds temporary Cloudflare TURN credentials to its ready frames,
keeping both the master API token and the temporary credentials out of share URLs.
Browsers prefer direct connections and can fall back to TURN. Native controllers
use outbound UDP, including to the browser owner's TURN relay; native TURN-over-
TCP/TLS allocation is not implemented. The development Rust service remains
STUN-only. Direct connections work where ICE
finds a viable path, including ordinary LAN and reachable tailnet addresses;
restrictive NATs/firewalls can prevent a connection. See the
[WebRTC connection guide](https://webrtc.org/getting-started/peer-connections)
for the distinction between signaling, STUN and TURN. A one-way URL with
automatic connection still needs rendezvous; eliminating that service requires
an out-of-band offer/answer exchange or an already reachable peer endpoint.

## Ownership and failure

Sharing closes the owner's healthy FF80 session before exporting its raw USB
interface. The owner's console is disabled while shared. The simulator remains
the same running guest and forwards ordinary USB bus operations through its
worker; it does not synthesize responses, patch guest RAM or inspect program
state to authorize hardware effects.

One remote connection owns the device at a time. Other clients may discover it
but cannot claim it while occupied. Normal Session.close closes the protocol
session and then releases the host lease. An error or abandoned claimed
connection fences the shared device; a new client cannot silently resume it.
The owner must stop sharing and reconnect. No timeout or disconnect retries a
camera command, resets USB, changes camera modes, or certifies that firmware
execution stopped. Stop sharing ends remote access; reconnect locally to resume
the owner's console.

The current stock simulator can time out on a subsequent session reopen after
a remote session has completed. This was observed with either Rust or a browser
as the later controller. Stop sharing and reconnect the simulator before another
controller session. The sharing layer reports and fences that failure; it does
not retry or patch the guest. Physical camera handoff has not been tested.

## Checks

Run Rust protocol, encrypted signaling and relay tests with
`cargo test -p fujimod-usb-proxy --features relay` and exporter tests with
`node --test crates/usb-proxy/web/exporter.test.mjs`.

`FUJIMOD_WEB_URL=<named HTTPS app URL> node crates/usb-proxy/web/peer.browser.mjs`
uses synthetic WebUSB with the real WebRTC/Rust path to test multi-megabyte
transfers, a Pyodide controller, exclusive ownership and fencing. No physical USB is accessed.

`FUJIMOD_WEB_URL=<named HTTPS app URL> node src/dev-tools/sharing.browser.mjs`
from `apps/web/` boots the configured stock-firmware simulator, checks the real
Share popup and clipboard, opens the link in a separate browser context,
executes a firmware console command remotely, exercises native Python with the
URL as its only USB environment setting on a separate cold simulator boot, and
checks link revocation for both runs. Proof
images and results go to ignored `apps/web/build/sharing-browser/`.

Worker runtime and forced-relay integration checks are described in
[`apps/usb-sharing`](../../apps/usb-sharing/README.md#test). The Workers runtime
checks use a mocked Cloudflare credentials API; live Cloudflare deployment and
account credentials are separate from those offline checks.

## Sharing status in Dev Tools

Agreed design, 2026-10-18; not built yet.

- **Top bar:** while the camera is shared, the status and Share button become
  one accent **Sharing** chip, with "· N connected" while remote controllers
  are attached. It opens the share dialog, which lists connected controllers
  with their connect reasons. **Stop sharing** beside it revokes the link and
  ends remote sessions but keeps the local camera connection; in local dev,
  auto-share then stays off until the camera reconnects or Share is pressed.
  **Disconnect** still ends everything.
- **Strip:** only while at least one remote controller is connected, one slim
  strip under the top bar, with no dialog, toast or dismiss button: "Your camera
  is being used remotely. Keep this tab in front and your computer on to stay
  connected." If the page was hidden during a remote session, the strip turns
  amber on return and says when the connection was interrupted.
- **Wake lock:** held only while a remote controller is connected; re-acquired
  when the tab becomes visible again; released when the last controller leaves.
- **Clean shutdown:** on page freeze or unload, stop sharing (controllers get a
  clean disconnect) and close the camera session. Switching tabs alone does not
  stop sharing.
