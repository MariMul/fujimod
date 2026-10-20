import type { BootInput, CardFile, CardReadOptions, EngineConfig, ReplayStep, Scene } from './api';
import type { EngineFault } from './fault';
import type { CameraPerformance } from './performance';
import type { SessionState } from './wasm-abi';

export type WasmRequest =
  | { action: 'prepare'; bundle: string }
  | {
      action: 'boot';
      dat: ArrayBuffer;
      config: EngineConfig;
      files: CardFile[];
      scene?: Scene;
      seedIdentity: { path: string; sha256: string }[];
    }
  | { action: 'press'; id: string; stage?: string; point?: { x: number; y: number } }
  | { action: 'release'; id: string }
  | { action: 'set_position'; id: string; position: string | number }
  | { action: 'scene'; scene: Scene | null }
  | { action: 'power'; on: boolean }
  | { action: 'power_cycle'; inputs: BootInput[] }
  | { action: 'card_state' | 'card_format' | 'save' | 'error_report' }
  | { action: 'card_tree'; revision?: number }
  | { action: 'card_read'; path: string; options?: CardReadOptions }
  | { action: 'card_insert'; present: boolean }
  | { action: 'restart_card'; files: CardFile[] }
  | { action: 'rollback'; seconds: number }
  | { action: 'profile'; limit?: number }
  | { action: 'replay'; steps: readonly ReplayStep[] };
export type WasmWorkerRequest =
  | { type: 'rpc'; serial: number; request: WasmRequest }
  | { type: 'frame_release'; lease: number };
export type WasmWorkerEvent =
  | { type: 'reply'; serial: number; result?: unknown }
  | { type: 'error'; serial?: number; error: string; fault?: EngineFault }
  | { type: 'progress'; message: string; percent?: number }
  | { type: 'state'; state: SessionState }
  | { type: 'metrics'; metrics: CameraPerformance }
  | { type: 'saved'; savedAt?: number; revision?: number; error?: string }
  | { type: 'frame'; bitmap: ImageBitmap; width: number; height: number; sequence: number }
  | {
      type: 'frame_view';
      buffer: SharedArrayBuffer;
      offset: number;
      length: number;
      width: number;
      height: number;
      sequence: number;
      lease: number;
    };
