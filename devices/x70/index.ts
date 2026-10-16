import { WorkerEngine } from '../../src/engine/worker-engine';
import parts from './parts.json';
export const initialPositions = { lcd_tilt: 'normal' };
export const model = { ...parts, url: 'devices/x70/x70.glb' };
export const createEngine = (onError: (error: Error) => void) => new WorkerEngine(
  () => new Worker(new URL('./worker.js', import.meta.url), { type: 'module' }), onError,
);
