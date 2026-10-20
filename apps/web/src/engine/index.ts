import type { Engine } from './api';
import { WasmEngine } from './wasm-engine';
export function createEngine(onError: (error: Error) => void): Engine {
  return new WasmEngine(onError);
}
