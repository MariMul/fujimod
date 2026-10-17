import type { Engine } from './api';
import { WorkerEngine } from './worker-engine';
export function createEngine(adapter: string, onError: (error: Error) => void): Engine {
  if (adapter !== 'pyodide') throw Error(`Unsupported emulator adapter: ${adapter}`);
  return new WorkerEngine(() => new Worker(new URL('./pyodide-worker.ts', import.meta.url), { type: 'module' }), onError);
}
