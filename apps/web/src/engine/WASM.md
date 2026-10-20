# Browser engine

The web app has one engine: the standalone Rust WebAssembly module, owned by a
module worker. `createEngine(onError)` exposes the typed Engine API. All CPU,
device, control, card, scene and history operations cross the versioned C ABI.
The production input table comes from the native `state.inputs` result.

`wasm.json` selects the module, binary, checked graph, guest tick rate, initial
run boundary and LCD output. The DAT is supplied separately and verified by the
module. A failed construction or unsupported native operation is an error.

The worker paces bounded runs against the guest clock. It presents borrowed
RGBA through OffscreenCanvas and transferred ImageBitmap, or leases a shared
Wasm view until the synchronous frame listener returns. Metrics count native
source events separately from delivered LCD frames. Host inspection never
advances the guest.

Native history is owner-bound. The worker retains eight explicit checkpoints
at two guest-second intervals; Undo uses native rollback, falling back to a
retained checkpoint when the finer native history has expired. CBOR history is
opaque to JavaScript. Checkpoints are distinct from durable camera saves;
unsupported persistence returns the module's error.

Development proof routes are opt-in. `/__wasm-proof` runs generated ARM fixtures.
`/__wasm-camera` reads explicitly configured local artifacts. Neither route
adds firmware to the production site. The ordinary app supports deterministic
development scenarios through `?scenario=x70-lcd` with a configured local DAT.
