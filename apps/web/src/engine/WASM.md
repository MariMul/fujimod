# WebAssembly engine

`createEngine({...config, adapter: 'wasm'}, onError)` selects the standalone
WebAssembly worker. `adapter: 'pyodide'` remains available. Selection is explicit:
the Wasm engine never falls back to Python or substitutes a simulated result for
a missing export. The application still uses only `api.ts`.

The bundle directory contains `wasm.json`, an Emscripten ES module loader and its
Wasm binary. The manifest has `schema: 1`, `abi: 1`, `module`, `wasm`,
`construction`, `session`, `ticksPerSecond`, `bootTicks` and `output`.
`module` and `wasm` resolve relative to the bundle URL. The loader accepts
`wasmBinary` and `locateFile`, and exposes the exports typed in `wasm-abi.ts`.
The engine hashes the actual module bytes to bind saved execution state to the
runtime. Firmware verification belongs to the module, before board creation.

`construction: 'resolved'` passes `session` directly to `camera_create` and uses
`config.inputs` for already-resolved `Input[]` metadata. Physical key IDs pass
directly to `camera_input`. Staged/touch input, storage, positions and power
require the session API and refuse if it is absent.

`construction: 'session'` uses `camera_session_create` and the module's own body
input metadata. The creation envelope is `{schema:1, graph:session, firmwareId,
bootSource, initialPositions, clockSeconds2000}`. There is no TypeScript device
map or firmware binding parser. `bootTicks` is an additional guest-time advance
after creation/restoration; use zero when creation already reaches its boot
boundary. The session implementation owns boot readiness and failure budgets.

## Pixel lifetime

`WasmSession.frame()` returns a borrowed RGBA8888 view onto the current heap.
Descriptor reads reacquire the heap after memory growth. There are no pixel
slices, RGB repacking, PNG encoding or structured clones of pixel arrays.

An unshared module cannot expose its memory across a worker boundary without a
copy. That path presents `ImageData` wrapping the borrowed heap view into an
`OffscreenCanvas`, then transfers an `ImageBitmap`. Canvas presentation performs
the browser's pixel upload; this is not an end-to-end zero-copy graphics claim.
The engine closes the bitmap after synchronous frame listeners return.

A shared module sends a view descriptor and a lease. The page constructs the
RGBA view over the same shared heap. The worker waits for the matching release
before processing another mutation. Listeners must finish reading synchronously;
they must copy explicitly to retain pixels. Shared Wasm builds require suitable
cross-origin isolation headers. The current unshared module does not qualify
this shared path in a real browser; transport unit tests cover it.

`setScene` accepts ordinary RGB bytes from the app, so it uploads them into the
span returned by `camera_scene_buffer`, then calls `camera_scene_commit`.
There is one explicit heap write, plus the browser's worker message transport.
The engine does not detach the caller's scene buffer. A producer that writes
directly to shared scene memory would need an additional lease contract.

## Session extension

The required optional exports are `camera_session_create`, `camera_command`,
`camera_scene_buffer` and `camera_scene_commit`. Missing exports reject calls.
`SessionCommands` in `wasm-abi.ts` is the typed JSON contract. A command receives
one `{action, options}` request and an optional opaque binary upload, and returns
one JSON result and an optional binary result. Binary spans used by card files,
saves and diagnostics are copied before their borrowed lifetime ends; they are
not frame buffers. Native inputs, card revision/busy checks, safe snapshots,
history/rollback and power transitions belong to the session implementation.

Owned saves use `CameraStore`'s transaction and optimistic revision checks.
Durable bytes and an optional safe snapshot come from one `save` operation;
there is no reuse of an older snapshot with newer storage. Snapshot reuse also
requires matching runtime, firmware, construction graph, boot source, mod state
and seed identities. Incompatible snapshots restore durable storage only;
`restart` forces that cold path. Live sessions save at boot and every 30 seconds,
retrying save failures after 10 seconds without stopping guest execution.
Stopped execution cannot replace the last good save. Explicit save failures
reject and publish a save error independently of emulator faults.

## Executable browser check

From `apps/web`, using the declared toolchains and a built module directory:

```sh
node scripts/prepare-wasm-proof.ts /path/to/module-directory
node node_modules/vite-plus/bin/vp dev --host 127.0.0.1 --port 5205
FUJIMOD_WEB_URL=http://127.0.0.1:5205 node tests/wasm.browser.ts
```

Artifacts are generated under ignored `public/runtime/wasm-proof*` and
`build/wasm-proof`. `/__wasm-proof` exists only on the development server.
It runs generated ARM code through the real worker, DAT decoder and modeled
LCD/ADC devices, presents both MENU states with the ordinary LCD viewer, checks
every pixel, rejects a modified DAT and reports a real unmapped-load fault.
The Chromium check asserts that no Python, NumPy or Pillow runtime is requested.
It does not qualify a camera boot or missing session capabilities.
