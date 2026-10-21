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
