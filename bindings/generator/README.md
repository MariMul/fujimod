# Shared binding generation

This Rust package derives language views from the public Rust service
declarations. It is a member of the root Cargo workspace and a build dependency
of both binding crates. The implementation crates stay independent of PyO3 and
wasm-bindgen.

The current generator reads `crates/usb/src/app80/catalogue.rs`, its value
modules, and the definition crate's `Codec` enum. It generates:

- Native Python and Wasm enum definitions and conversions.
- Command/action factories and nested structured reply variants, including
  discriminated TypeScript unions and owned buffers/arrays.
- Unit result variants with an `Unknown(u8)` payload, including TypeScript unions.
- Checked scalar and byte-buffer wrappers that call the original Rust constructor.
- Native registrations, JavaScript namespace exports and Pyodide value metadata.
- Distinct TypeScript handle types using a private declaration brand, so unrelated
  Wasm classes cannot substitute for each other through structural typing.

For these shapes, add the public Rust declaration and its usual Rust exports.
There is no second list of enum variants, validation bounds or value exports in
the targets. Checked values use `new(value: u8/u16/u32)` or `new(data: &[u8])`
returning `Result<Self, CommandError>`, with a public `value()` or `data()` getter.
Payload enum variants use named Rust fields; unit variants need no fields.
The generator follows the types reachable from `Command` and `ReplyValue` to
distinguish factory inputs from reply outputs. Field names define Python
constructor keywords and JavaScript properties. New variants and nested enums
reach every target without another export or conversion list. The existing
`WordAddress` bridge also handles the core address wrapper.
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

Session adapters remain handwritten: async ownership, cancellation, error
translation and buffer conversion live in shared target adapters. A new field
shape can require extending the generator once. Unsupported or unclassified
payload types fail generation with a diagnostic rather than being silently
omitted. Command validation and wire encoding remain ordinary Rust code.
