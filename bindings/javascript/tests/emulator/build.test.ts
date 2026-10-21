import assert from 'node:assert/strict';
import { test } from 'node:test';
import { buildPlan, options, pinnedToolchain, shellWords } from '../../emulator/build.mts';

test('builder reads the pinned public toolchains from the package root configuration', () => {
  assert.deepEqual(pinnedToolchain(), { nightly: 'nightly-2026-10-10', emsdk: '5.0.3' });
});

test('build options keep artifacts in the requested output and validate tool flags', () => {
  const selected = options(
    ['--out', 'out/wasm', '--rust-simd128', '--c-simd128', '--allocator', 'emmalloc'],
    '/tmp/project',
  );
  assert.equal(selected.out, '/tmp/project/out/wasm');
  assert.equal(selected.rustSimd128, true);
  assert.equal(selected.cSimd128, true);
  assert.equal(selected.allocator, 'emmalloc');
  assert.throws(() => options(['--lto', 'invalid']), /Invalid --lto/);
  assert.throws(() => options(['--codegen-units', '0']), /positive integer/);
});

test('build plan targets the public static library and preserves encoded Rust flags', () => {
  const selected = options(['--out', '/tmp/emulator', '--rust-simd128'], '/tmp/project');
  const plan = buildPlan(selected, {
    CARGO_ENCODED_RUSTFLAGS: '-C\x1fdebuginfo=0',
    PATH: '/toolchain/bin',
  });
  assert.equal(plan.env.CARGO_TARGET_DIR, '/tmp/emulator/rust');
  assert.equal(plan.env.CAMERA_UNICORN_STATIC_DIR, '/tmp/emulator/unicorn');
  assert.deepEqual(plan.rustFlags, ['-C', 'debuginfo=0', '-C', 'target-feature=+atomics,+bulk-memory,+mutable-globals', '-C', 'target-feature=+simd128']);
  assert(plan.cFlags.includes('-pthread'));
  assert(plan.link.includes('-pthread'));
  assert(plan.link.includes('-sPTHREAD_POOL_SIZE=2'));
  assert(plan.rust.includes('-Zbuild-std'));
  assert(plan.rust.at(-1)?.endsWith('/bindings/javascript/emulator/Cargo.toml'));
  assert(
    plan.link.includes(
      '/tmp/emulator/rust/wasm32-unknown-emscripten/release/libfujimod_emulator_javascript.a',
    ),
  );
  assert(
    plan.link.some((argument) => argument.endsWith('/bindings/javascript/emulator/exports.json')),
  );
  assert(plan.link.includes('-sENVIRONMENT=web,worker,node'));
});

test('Rust flag tokenizer honors quoted groups without evaluating shell syntax', () => {
  assert.deepEqual(shellWords(`-C 'link-arg=-Wl,-u,symbol name' --cfg "feature=jit"`), [
    '-C',
    'link-arg=-Wl,-u,symbol name',
    '--cfg',
    'feature=jit',
  ]);
  assert.throws(() => shellWords("-C 'unfinished"), /Unclosed flag quote/);
});
