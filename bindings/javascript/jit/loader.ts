// Transport adapter; defaults, validation, compilation and policy live in Rust.
interface JitModule {
  postRun?: (() => void) | (() => void)[];
  tcgJit?: unknown;
  executionPolicy?: 'interactive' | 'exact';
  _camera_policy_setup(pointer: number, length: number): number;
  tcgJitStats?: () => unknown;
  _camera_alloc(length: number): number;
  _camera_free(pointer: number, length: number): void;
  _camera_jit_setup(pointer: number, length: number): number;
  _camera_jit_stats(out: number): number;
  _camera_error(handle: number, out: number): number;
}
declare const Module: JitModule;
declare const wasmMemory: WebAssembly.Memory;
declare const ENVIRONMENT_IS_PTHREAD: boolean;
declare const HEAPU8: Uint8Array;
if (typeof ENVIRONMENT_IS_PTHREAD === 'undefined' || !ENVIRONMENT_IS_PTHREAD) {
  const previous = Module.postRun;
  const callbacks = Array.isArray(previous) ? previous.slice() : previous ? [previous] : [];
  Module.postRun = callbacks;
  callbacks.push(() => {
    // pre-js may run before Emscripten initializes its environment flags.
    if (typeof ENVIRONMENT_IS_PTHREAD !== 'undefined' && ENVIRONMENT_IS_PTHREAD) return;
    const decoder = new TextDecoder();
    // A device pthread can grow shared memory without executing JavaScript on
    // this owner. Export a current view rather than retaining its old bounds.
    let exposedHeap: Uint8Array = new Uint8Array(wasmMemory.buffer);
    Object.defineProperty(Module, 'HEAPU8', {
      configurable: true,
      get: () => {
        if (exposedHeap.buffer !== wasmMemory.buffer)
          exposedHeap = new Uint8Array(wasmMemory.buffer);
        return exposedHeap;
      },
      set: (heap: Uint8Array) => { exposedHeap = heap; },
    });
    const span = (out: number) => {
      const view = new DataView(wasmMemory.buffer),
        pointer = view.getUint32(out, true),
        length = view.getUint32(out + 4, true);
      return decoder.decode(HEAPU8.slice(pointer, pointer + length));
    };
    const check = (code: number) => {
      if (code === 0) return;
      const out = Module._camera_alloc(8);
      try {
        Module._camera_error(0, out);
        throw Error(span(out));
      } finally {
        Module._camera_free(out, 8);
      }
    };
    const policy = new TextEncoder().encode(Module.executionPolicy ?? 'interactive');
    const policyPointer = Module._camera_alloc(policy.length);
    try { HEAPU8.set(policy, policyPointer); check(Module._camera_policy_setup(policyPointer, policy.length)); }
    finally { Module._camera_free(policyPointer, policy.length); }
    const bytes =
      Module.tcgJit === undefined
        ? new Uint8Array()
        : new TextEncoder().encode(JSON.stringify(Module.tcgJit) ?? 'null');
    const pointer = bytes.length ? Module._camera_alloc(bytes.length) : 0;
    try {
      if (bytes.length) HEAPU8.set(bytes, pointer);
      check(Module._camera_jit_setup(pointer, bytes.length));
    } finally {
      if (bytes.length) Module._camera_free(pointer, bytes.length);
    }
    Module.tcgJitStats = () => {
      const out = Module._camera_alloc(8);
      try {
        check(Module._camera_jit_stats(out));
        return JSON.parse(span(out));
      } finally {
        Module._camera_free(out, 8);
      }
    };
  });
}
