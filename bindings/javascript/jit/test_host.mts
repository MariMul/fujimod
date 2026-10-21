// The TS boundary forwards configuration and loads Rust-requested exports.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { runInNewContext } from 'node:vm';
const source = (name: string) =>
  stripTypeScriptTypes(readFileSync(new URL(name, import.meta.url), 'utf8'), { mode: 'strip' });
const memory = new WebAssembly.Memory({ initial: 4 }),
  table = new WebAssembly.Table({ initial: 2, element: 'anyfunc' });
let library: Record<string, unknown> = {};
const module: Record<string, unknown> = {
  _uc_embedder_tci_wasm_load: (...args: number[]) => {
    assert.deepEqual(args, [7, 8, 0, 9]);
    return 42n;
  },
};
const context = {
  Module: module,
  wasmMemory: memory,
  wasmTable: table,
  HEAPU8: new Uint8Array(memory.buffer),
  TextEncoder,
  TextDecoder,
  DataView,
  WebAssembly,
  performance: { now: () => 123.5 },
  addToLibrary: (value: Record<string, unknown>) => {
    library = value;
  },
};
runInNewContext(source('./host.ts'), context);
const grow = library.unicorn_jit_host_grow as (count: number) => number;
const install = library.unicorn_jit_host_install as (
  pointer: number,
  length: number,
  exports: number,
  count: number,
) => number;
const clear = library.unicorn_jit_host_clear as (slot: number) => void;
const error = library.unicorn_jit_host_error as (pointer: number, length: number) => number;
assert.equal(grow(2), 2);
assert.equal(table.length, 4);
// A single handwritten Wasm fixture forwards its arguments to env.load.
const text = (value: string) => [value.length, ...new TextEncoder().encode(value)];
const section = (id: number, body: number[]) => [id, body.length, ...body];
const body = [0, 0x20, 0, 0x20, 1, 0x41, 0, 0x20, 2, 0x10, 0, 0x0b];
const fixture = new Uint8Array([
  0,
  97,
  115,
  109,
  1,
  0,
  0,
  0,
  ...section(1, [2, 0x60, 3, 0x7f, 0x7f, 0x7f, 1, 0x7e, 0x60, 4, 0x7f, 0x7f, 0x7f, 0x7f, 1, 0x7e]),
  ...section(2, [1, ...text('env'), ...text('load'), 0, 1]),
  ...section(3, [1, 0]),
  ...section(7, [1, ...text('fn'), 0, 1]),
  ...section(10, [1, body.length, ...body]),
]);
context.HEAPU8.set(fixture, 0x100);
context.HEAPU8.set(new TextEncoder().encode('fn'), 0x800);
const view = new DataView(memory.buffer);
[0x800, 2, 2].forEach((value, n) => view.setUint32(0x900 + n * 4, value, true));
assert.equal(install(0x100, fixture.length, 0x900, 1), 0);
assert.equal((table.get(2) as (a: number, b: number, c: number) => bigint)(7, 8, 9), 42n);
view.setUint32(0x904, 1, true);
assert.equal(install(0x100, fixture.length, 0x900, 1), -1);
assert.equal((table.get(2) as (a: number, b: number, c: number) => bigint)(7, 8, 9), 42n);
const length = error(0, 0);
assert(length > 0);
assert.equal(error(0xa00, length), length);
assert.match(
  new TextDecoder().decode(context.HEAPU8.subarray(0xa00, 0xa00 + length)),
  /missing function export/,
);
clear(2);
assert.equal(table.get(2), null);
assert.equal((library.unicorn_jit_host_now as () => number)(), 123.5);

function loader(value: unknown, reject = false) {
  const memory = new WebAssembly.Memory({ initial: 4 });
  let next = 0x10000,
    oldHook = 0,
    grow = true;
  const allocated = new Map<number, number>(),
    configs: string[] = [];
  const expected = { enabled: true, compiled: 7, entries: '123', options: { threshold: 19 } };
  const state = {
    Module: {} as Record<string, unknown>,
    wasmMemory: memory,
    HEAPU8: new Uint8Array(memory.buffer),
    TextEncoder,
    TextDecoder,
    DataView,
  };
  const write = (out: number, text: string) => {
    const bytes = new TextEncoder().encode(text);
    state.HEAPU8.set(bytes, 0x1000);
    const view = new DataView(memory.buffer);
    view.setUint32(out, 0x1000, true);
    view.setUint32(out + 4, bytes.length, true);
  };
  state.Module = {
    postRun: () => {
      oldHook++;
    },
    _camera_alloc: (length: number) => {
      const pointer = next;
      next += (length + 7) & -8;
      allocated.set(pointer, length);
      return pointer;
    },
    _camera_free: (pointer: number, length: number) => {
      assert.equal(allocated.get(pointer), length);
      allocated.delete(pointer);
    },
    _camera_jit_setup: (pointer: number, length: number) => {
      configs.push(new TextDecoder().decode(state.HEAPU8.subarray(pointer, pointer + length)));
      return reject ? -1 : 0;
    },
    _camera_jit_stats: (out: number) => {
      if (grow) {
        grow = false;
        memory.grow(1);
        state.HEAPU8 = new Uint8Array(memory.buffer);
      }
      write(out, JSON.stringify(expected));
      return 0;
    },
    _camera_error: (_handle: number, out: number) => {
      write(out, 'config refused');
      return 0;
    },
  };
  if (value !== undefined) state.Module.tcgJit = value;
  runInNewContext(source('./loader.ts'), state);
  const callbacks = state.Module.postRun as (() => void)[];
  if (reject) {
    assert.throws(() => callbacks.forEach((fn) => fn()), /config refused/);
  } else {
    callbacks.forEach((fn) => fn());
    assert.deepEqual(
      JSON.parse(JSON.stringify((state.Module.tcgJitStats as () => unknown)())),
      expected,
    );
  }
  assert.equal(oldHook, 1);
  assert.equal(allocated.size, 0);
  assert.deepEqual(configs, [value === undefined ? '' : JSON.stringify(value)]);
}
for (const value of [undefined, false, true, { threshold: 19, unknown: 'forwarded to Rust' }])
  loader(value);
loader(true, true);
console.log(
  JSON.stringify({
    passed: true,
    hostImports: true,
    rustExportManifest: true,
    configurationTransport: true,
    memoryGrowth: true,
    borrowedStatistics: true,
    errorCleanup: true,
  }),
);
