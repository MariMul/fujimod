import sourceArchiveLoader from './source-archive.py?raw';
import { cachedFetch } from '../asset-cache';
import { boundedMemoryModule, type RuntimeSettings } from './wasm-growth';
interface PyBuffer {
  data: Uint8Array;
  release(): void;
}
export interface PyProxy {
  getBuffer(kind: string): PyBuffer;
  destroy(): void;
}
export interface Pyodide {
  _module: { HEAPU8: Uint8Array };
  loadPackage(packages: string[]): Promise<void>;
  unpackArchive(data: ArrayBuffer, format: string, options: { extractDir: string }): void;
  FS: {
    mkdirTree(path: string): void;
    writeFile(path: string, data: Uint8Array, options: { canOwn: boolean }): void;
  };
  globals: {
    set(name: string, value: unknown): void;
    delete(name: string): void;
  };
  runPython<T = string>(source: string): T;
  toPy(value: Uint8Array): PyProxy;
}
export async function loadCameraRuntime(bundle: string) {
  let py: Pyodide;
  const originalFetch = self.fetch.bind(self);
  const runtimeRoot = new URL(bundle).href;
  self.fetch = (input, init) => {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
    const method = init?.method || (input instanceof Request ? input.method : 'GET');
    return method === 'GET' && url.startsWith(runtimeRoot)
      ? cachedFetch(url, originalFetch)
      : originalFetch(input, init);
  };
  try {
    const assets = Promise.all(
      ['sources.zip', '_native.so'].map(async (name) => {
        const response = await fetch(new URL(name, bundle));
        if (!response.ok) throw Error('Emulator bundle is unavailable.');
        return response.arrayBuffer();
      }),
    );
    const { loadPyodide } = (await import(
      /* @vite-ignore */ new URL('pyodide/pyodide.mjs', bundle).href
    )) as {
      loadPyodide(
        this: void,
        options: {
          indexURL: string;
          packages: string[];
          createPyodideModule: unknown;
        },
      ): Promise<Pyodide>;
    };
    py = await loadPyodide({
      indexURL: new URL('pyodide/', bundle).href,
      packages: ['numpy', 'pillow'],
      createPyodideModule: async (settings: RuntimeSettings) => {
        const { default: factory } = await import(
          /* @vite-ignore */ new URL('pyodide/pyodide.asm.mjs', bundle).href
        );
        return boundedMemoryModule(factory)(settings);
      },
    });
    const [sources, native] = await assets;
    py.FS.mkdirTree('/repo/native/python/camera_native');
    py.FS.writeFile('/repo/runtime-sources.zip', new Uint8Array(sources), { canOwn: true });
    py.FS.writeFile('/repo/native/python/camera_native/_native.so', new Uint8Array(native), {
      canOwn: true,
    });
    py.runPython(sourceArchiveLoader);
    py.runPython(
      "import os,sys; os.chdir('/repo'); sys.path[:0]=['/repo/native/python','/repo','/repo/tools']\nsource_archive_finder = install_source_archive('/repo/runtime-sources.zip','/repo',('native/python/','','tools/'))",
    );
    return { py, sources, native };
  } finally {
    self.fetch = originalFetch;
  }
}
