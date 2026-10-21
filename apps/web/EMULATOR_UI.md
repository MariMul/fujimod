# Emulator UI

The emulator UI (viewer, control panel, card browser, fault and report UI,
status) is one package used by every frontend: the site's homepage, Dev Tools
and a small standalone frontend served by a native emulator process.

## Boundaries

- The UI drives an emulator only through `Engine` (`src/engine/api.ts`).
- `WasmEngine` runs the emulator in a browser worker. `RemoteEngine` reaches a
  native emulator process. Both send the same messages: the worker protocol
  becomes the wire protocol, defined once and used by the worker, the remote
  engine and the native process.
- A session controller owns one running emulator: boot, inputs, card, faults,
  checkpoints and Undo, using only `Engine`. The homepage adds its own parts
  around it: saved cameras, catalog, firmware choice and site pages.
- Control layouts are data (`DevicePanel`). The same renderer draws them in
  every frontend.

## Transport

WebRTC only. A native emulator process serves the UI bundle over HTTP, and the
same server exchanges the WebRTC offer and answer. Remote connections use the
existing share-link signaling. Control and card messages use a data channel;
the LCD is a video track. The native and browser peers are the ones in
`crates/usb-proxy`.

## Control layouts

A layout file is a versioned `DevicePanel` naming its body. Presets ship per
body; bodies without one get a layout generated from the emulator's input
list. Import checks every bound control against that list and reports unknown
ones. The editor edits the same tree in place on the live panel and exports
it as JSON. User layouts are stored with the saved camera.

## Order

1. Split `src/home/runtime.ts` into the session controller and homepage code.
2. Move the UI into its package; the homepage uses it.
3. Layout schema version, import/export, binding checks, then the editor.
4. `RemoteEngine` over WebRTC, and the native process serving the UI.

## Refactor rules

Each step is a real change of structure, checked in review:

- Code moves; it is not copied. Old modules are deleted in the same commit,
  with no re-exports, wrappers or aliases left behind, and every importer
  updated.
- One implementation per concept: one session controller, one protocol
  definition, one layout renderer. A second frontend reuses them; it does not
  get its own variant.
- No behavior changes ride along with a move. Existing unit and browser checks
  pass unchanged, except for import paths.
- Each step's commit message lists the deleted modules. Review checks that no
  remaining file imports them and that no moved function exists in two places.
