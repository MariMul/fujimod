import type { BootProtocol, EngineConfig, Scene } from './api';
export const testBoot: BootProtocol = {
  datPath: '/firmware.dat',
  prepare: { module: 'test', function: 'prepare', options: {} },
  module: 'test',
  sessionModule: 'test',
  limit: 1,
  step: 1,
  readyLimit: 1,
  readyWhen: [],
  sceneInput: true,
};
export function testConfig(sceneInput = true): EngineConfig {
  return {
    firmwareId: 'test',
    inputs: 'inputs.json',
    bundle: 'runtime/',
    boot: { ...testBoot, sceneInput },
  };
}
export interface TestMessage {
  type: string;
  id: number;
  action: string;
  revision: number;
  path: string;
  maxBytes: number;
  cardFiles: { path: string; bytes: ArrayBuffer }[];
  present: boolean;
  replaceCard: boolean;
  bootInputs: import('./api').BootInput[];
  seconds: number;
  config: EngineConfig;
  initialPositions: Record<string, string>;
  options: { pause_at_tick: number; pixels: boolean };
  scene: Scene;
  rgb: ArrayBuffer;
  transfer?: Transferable[];
  files: { path: string; bytes: ArrayBuffer }[];
  seedIdentity: { path: string; sha256: string }[];
}
export function fakeWorker(worker: unknown): Worker {
  return worker as Worker;
}
export function fakeRuntime(value: unknown): import('./recovery-worker').RecoveryRuntime {
  return value as import('./recovery-worker').RecoveryRuntime;
}
