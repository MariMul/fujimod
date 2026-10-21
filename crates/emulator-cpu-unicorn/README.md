# Unicorn CPU backend

This backend loads the pinned Unicorn source through the C API. Initialize the
submodule and build its native ARM library with CMake and Ninja:

```sh
git submodule update --init -- crates/emulator-cpu-unicorn/vendor/unicorn
cmake -S crates/emulator-cpu-unicorn/vendor/unicorn \
  -B target/unicorn-native -G Ninja \
  -DUNICORN_ARCH=arm \
  -DUNICORN_TCG_INTERPRETER=OFF \
  -DUNICORN_BUILD_TESTS=OFF \
  -DUNICORN_INSTALL=OFF
cmake --build target/unicorn-native --target unicorn --parallel 4
```

The Rust tests load the built shared library from `CAMERA_UNICORN_LIBRARY` and
check the required embedder exports. On macOS, the build above produces
`target/unicorn-native/libunicorn.2.dylib`:

```sh
CAMERA_UNICORN_LIBRARY="$PWD/target/unicorn-native/libunicorn.2.dylib" \
  cargo test --locked -p fujimod-emulator-cpu-unicorn
```

On other platforms, set `CAMERA_UNICORN_LIBRARY` to the shared library path
produced by CMake.

## Exact loop execution

Exact board runs use `LoopMode::Accelerate` by default. Select
`LoopMode::Measure` to count eligible instructions while executing them, or
`LoopMode::Disabled` for the ordinary backend comparison.
The board grants each exact run a stable window ending at its first device,
DMA, interrupt or runnable-peer boundary. A standalone CPU owner must grant
that window explicitly; it expires when the run returns. Measurement executes
every instruction and counts qualifying taken iterations. Acceleration charges
the same instructions and leaves partial iterations to ordinary execution.

The conservative A32 proof accepts direct backward loops of at most sixteen
instructions, unconditional integer operations, aligned RAM/declared pure
status loads, fixed architectural states and unit-step counters compared with
an invariant immediate or register. It rebuilds the proof from fetch-checked
bytes, checks data permissions, preserves NZCV, and stops before counter wrap.
Stores, atomics, system operations, conditional bodies, Thumb and unsupported
algebra continue through the CPU normally. Instruction/block observers inside
a loop, read observers, discovery, transaction recording and watched device
waits disable acceleration. Loop counters and worklist visit stamps are host
bookkeeping; pending work, deadlines and all modeled device state remain in
the machine fingerprint.

