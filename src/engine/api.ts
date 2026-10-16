import type { CameraPerformance } from './performance';
import type { EngineFault } from './fault';
export type { EngineFault } from './fault';
/** The only emulator contract used by the website. All scheduling is engine-owned. */
export interface Input {
  id: string;
  label: string;
  kind: string;
  supported: boolean;
  positions?: { id: string; label: string }[];
  initial?: string;
  relative?: boolean;
  stages?: { id: string; label: string }[];
}
export interface Frame {
  rgb: Uint8Array;
  width: number;
  height: number;
  sequence: number;
}
export interface Scene { rgb: Uint8Array; width: number; height: number }
export interface CardFile { path: string; bytes: Uint8Array }
export interface CardState {
  supported: boolean;
  present: boolean;
  busy: boolean;
  revision: number | null;
  capacity_bytes: number;
}
export interface CardEntry {
  path: string;
  name: string;
  kind: 'file' | 'directory';
  bytes: number;
  children?: CardEntry[];
}
export interface CardTree extends CardState { entries: CardEntry[] }
export interface CardReadOptions { revision?: number; maxBytes?: number }
export interface BootProtocol {
  datPath: string;
  prepare: { module: string; function: string; options: Record<string, unknown> };
  module: string;
  sessionModule: string;
  limit: number;
  step: number;
  readyLimit: number;
  readyWhen: string[];
}
export interface EngineConfig {
  bundle: string;
  inputs: string;
  boot: BootProtocol;
  initialPositions?: Record<string, string>;
  restart?: boolean;
}
export interface Engine {
  metrics(listener: (metrics: CameraPerformance) => void): () => void;
  profile(limit?: number): Promise<Record<string, unknown>>;
  errors(listener: (fault: EngineFault) => void): () => void;
  rollback(seconds: number): Promise<void>;
  boot(dat: ArrayBuffer, config: EngineConfig, progress: (message: string, percent: number) => void, cardFiles?: CardFile[]): Promise<Input[]>;
  /** Restart with these files merged into existing media; matching paths are replaced. */
  restartWithCardFiles(files: CardFile[], progress: (message: string, percent: number) => void): Promise<void>;
  cardState(): Promise<CardState>;
  cardTree(revision?: number): Promise<CardTree>;
  readCardFile(path: string, options?: CardReadOptions): Promise<Uint8Array>;
  /** Media changes, including busy transitions; refresh the tree after writes settle. */
  cardChanges(listener: (state: CardState) => void): () => void;
  frames(listener: (frame: Frame) => void): () => void;
  press(id: string, stage?: string, point?: { x: number; y: number }): Promise<void>;
  release(id: string): Promise<void>;
  setPosition(id: string, position: string | number): Promise<void>;
  setScene(scene: Scene | null): Promise<void>;
  power(on: boolean): Promise<void>;
  dispose(): void;
}
