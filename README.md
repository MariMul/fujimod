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

The root `web-build`, `web-test` and `web-preview` mise tasks run from that
application directory. The Pages workflow builds `apps/web` from approved
main releases. Review branches do not deploy Pages.

## Rust libraries

The root Cargo workspace contains reusable implementation crates under `crates/`.
The `fujimod` facade exposes components through optional features; enable `usb`
for `fujimod::usb`. The [mod installation crate](crates/mod-install/README.md)
provides checked uploads and borrowed callback batches. The [USB crate](crates/usb/README.md) provides the initial
asynchronous service/debug protocol implementation.
The [catalog crate](crates/catalog/README.md) loads caller-selected firmware
data through `fujimod::catalog`, shared by USB and the emulator.
Enable the `emulator` feature for `fujimod::emulator`; execution, device models
and processor backends live in separate crates under `crates/emulator*`.
The Python package exposes the same execution core through `fujimod.emulator`.

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The core also builds for WebUSB. See the USB crate's
[Wasm checks](crates/usb/README.md#wasm-core-checks) for the JS-runtime tests;
the [JavaScript package](bindings/javascript/README.md) provides typed browser
APIs over the same core. Python in Pyodide uses the
[WebUSB adapter](bindings/python/PYODIDE.md) with that initialized module.

The [Python package](bindings/python/README.md) lives in `bindings/python/`.

Rust owns each browser USB device for the page's lifetime. Local sessions and
remote controllers are both clients of that one nusb owner, with one session
ownership lock. The browser end runs the Rust/Wasm exporter; JavaScript only
routes its network messages. Sharing attaches or detaches peers independently
of the physical connection. See [USB sharing](crates/usb-proxy/SHARING.md).

The JavaScript/Wasm package lives in `bindings/javascript/`. Binding crates are
members of this same workspace. Each directory owns its language package;
implementation crates remain independent of PyO3 and wasm-bindgen. The browser
application stays in `apps/web/` and consumes the JavaScript package as needed.

The [shared binding generator](bindings/generator/README.md) derives service
value wrappers, command/action factories, structured replies and language exports
from Rust declarations. Routine service features need only Rust code; shared
adapters handle execution and language boundaries.

## Continuous integration

See [CI.md](CI.md) for the required checks, local commands, coverage reports and
the distinction between public tests and firmware-dependent rehearsals.
