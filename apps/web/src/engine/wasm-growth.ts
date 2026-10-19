/** The pinned Pyodide factory supplies Emscripten's instantiation hook.
 * Keep its allocator and view refresh intact, but replace 20% heap overgrowth
 * (up to 96 MiB per expansion) with a 4 MiB reserve. Wasm cannot shrink. */
export interface RuntimeSettings {
  instantiateWasm(imports: { env: Record<string, unknown> }, receive: unknown): unknown;
  growMemory?(bytes: number): number;
}
export function boundedMemoryModule<T>(factory: (settings: RuntimeSettings) => Promise<T>) {
  return (settings: RuntimeSettings) => {
    const instantiate = settings.instantiateWasm;
    settings.instantiateWasm = (imports, receive) => {
      if (typeof imports.env.emscripten_resize_heap !== 'function')
        throw Error('The runtime heap-growth ABI differs from this engine.');
      imports.env.emscripten_resize_heap = (requested: number) => {
        if (!settings.growMemory) throw Error('The runtime cannot refresh grown memory views.');
        requested >>>= 0;
        const exact = Math.ceil(requested / 65536) * 65536;
        const reserve = Math.min(0x100000000, exact + 4 * 1024 * 1024);
        return settings.growMemory(reserve) || settings.growMemory(exact);
      };
      return instantiate(imports, receive);
    };
    return factory(settings);
  };
}
