import type { Engine } from './api';
import { DefaultSceneEngine, loadImageScene } from './default-scene-engine';
export function createEngine(adapter: string, onError: (error: Error) => void): Engine {
  if (adapter !== 'pyodide') throw Error(`Unsupported emulator adapter: ${adapter}`);
  return new DefaultSceneEngine(
    () => new Worker(new URL('./pyodide-worker.ts', import.meta.url), { type: 'module' }),
    onError,
    () => loadImageScene(new URL('./assets/default-dunes.png', import.meta.url)),
  );
}
