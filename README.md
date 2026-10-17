# FujiMod

Camera tools, emulation and firmware mods. The browser application lives in
[apps/web](apps/web/README.md), including its body definitions, patch
interpreter, runtime adapter and browser checks.

Use `mise install`, then work in `apps/web`:

```sh
cd apps/web
npm ci
npm test
npm run dev
```

The root `site-build`, `site-test` and `site-preview` mise tasks run from that
application directory. The Pages workflow builds `apps/web` from approved
main releases. Review branches do not deploy Pages.

## Rust libraries

The root Cargo workspace contains reusable implementation crates under `crates/`.
The `fujimod` facade exposes components through optional features; enable `usb`
for `fujimod::usb`. The [USB crate](crates/usb/README.md) provides the initial
asynchronous service/debug protocol implementation.

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Language bindings will live in `bindings/python/` and `bindings/javascript/` as
members of this same workspace. Each directory owns its language package;
implementation crates remain independent of PyO3 and wasm-bindgen. The browser
application stays in `apps/web/` and consumes the JavaScript package as needed.
