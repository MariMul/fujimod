# Shared binding generation

This Rust package derives language views from the public Rust service value
declarations. It is a member of the root Cargo workspace and a build dependency
of both binding crates. The implementation crates stay independent of PyO3 and
wasm-bindgen.

The current generator reads `crates/usb/src/app80/catalogue.rs`, its value
modules, and the definition crate's `Codec` enum. It generates:

- Native Python and Wasm enum definitions and conversions.
- Unit result variants with an `Unknown(u8)` payload, including TypeScript unions.
- Checked scalar and byte-buffer wrappers that call the original Rust constructor.
- Native registrations, JavaScript namespace exports and Pyodide value metadata.

For these shapes, add the public Rust declaration and its usual Rust exports.
There is no second list of enum variants, validation bounds or value exports in
the targets. Checked values use `new(value: u8/u16/u32)` or `new(data: &[u8])`
returning `Result<Self, CommandError>`, with a public `value()` or `data()` getter.
The existing `WordAddress` bridge also handles the core address wrapper.
Validation and command bytes remain in the implementation crate.

Cargo build scripts generate Rust into `OUT_DIR`. The JavaScript build generates
`pkg/service-values.js` and its declarations after wasm-bindgen. Generated files
are build output. The browser wheel's shared Python adapter consumes metadata
from the initialized JavaScript package through `registerPyodide`; it does not
need a separate generated Python source tree.

```sh
cargo test -p fujimod-bindgen
npm --prefix bindings/javascript test
```

Command/action factories, structured replies beyond these result shapes and
session adapters are still handwritten. Extending the generator to those is the
next migration step. Async ownership, cancellation, errors and buffer conversion
stay in the shared target adapters rather than becoming per-feature code.
