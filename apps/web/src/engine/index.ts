import type { Engine, EngineConfig } from './api';
import { DefaultSceneEngine } from './default-scene-engine';
import { loadImageScene } from './image-scene';
import { WorkerEngine } from './worker-engine';
import { WasmEngine } from './wasm-engine';
export function createEngine(
  config: EngineConfig & { adapter: string },
  onError: (error: Error) => void,
): Engine {
  if (config.adapter === 'wasm') return new WasmEngine(onError);
  if (config.adapter !== 'pyodide') throw Error(`Unsupported emulator adapter: ${config.adapter}`);
  const worker = () =>
    new Worker(new URL('./pyodide-worker.ts', import.meta.url), { type: 'module' });
  if (!config.boot.sceneInput) return new WorkerEngine(worker, onError);
  return new DefaultSceneEngine(worker, onError, () =>
    loadImageScene(new URL('./assets/default-dunes.png', import.meta.url)),
  );
}
