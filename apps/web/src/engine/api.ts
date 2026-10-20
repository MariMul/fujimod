import type { ReportSubmission } from './report';
import type { CameraPerformance } from './performance';
import type { EngineFault } from './fault';
export type { EngineFault } from './fault';
/** The only emulator contract used by the web app. All scheduling is engine-owned. */
export interface Input {
  id: string;
  label: string;
  kind: string;
  supported: boolean;
  unavailable?: string;
  positions?: { id: string; label: string }[];
  initial?: string;
  relative?: boolean;
  width?: number;
  height?: number;
  stages?: { id: string; label: string }[];
}
export interface Connection {
  id: string;
  label: string;
  connection: 'connected' | 'disconnected' | 'unknown';
  support: 'modeled' | 'partial' | 'unmodeled';
  limits: string;
}
/** Frames are borrowed only during the synchronous listener call. */
export type Frame = {
  width: number;
  height: number;
  sequence: number;
} & ({ rgb: Uint8Array } | { rgba: Uint8ClampedArray } | { bitmap: ImageBitmap });
export interface Scene {
  rgb: Uint8Array;
  width: number;
  height: number;
}
export interface CardFile {
  path: string;
  bytes: Uint8Array;
}
export interface BootMedia {
  identities: Promise<{ path: string; sha256: string }[]>;
  load(): Promise<CardFile[]>;
}
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
export interface CardTree extends CardState {
  entries: CardEntry[];
}
export interface CardReadOptions {
  revision?: number;
  maxBytes?: number;
}
export interface BootInput {
  id: string;
  stage?: string;
  position?: string;
}
export type TouchPhase = 'press' | 'drag' | 'release' | 'tap';
export type ReplayStep =
  | { action: 'press' | 'release'; id: string; stage?: string }
  | { action: 'set_position'; id: string; position: string }
  | { action: 'rotate'; id: string; steps: number }
  | { action: 'touch'; id: string; phase: TouchPhase; x: number; y: number }
  | { action: 'advance'; ticks: number };
export interface EngineConfig {
  camera?: { id: string; revision: number; modEnabled: boolean };
  firmwareId: string;
  bundle: string;
  sceneInput: boolean;
  initialPositions?: Record<string, string>;
  restart?: boolean;
  /** Physical contacts held before the first instruction of a cold boot. */
  bootInputs?: BootInput[];
  /** Cold boot with a fixed RTC; replay mode advances only through replay(). */
  execution?: { mode: 'live' | 'replay'; clockSeconds2000?: number };
}
export interface Checkpoint {
  id: string;
  tick: string;
  instructions: string[];
}
export interface ReplayLog {
  format: 'session-input-log-cbor';
  schema: 1;
  entries: number;
  bytes: Uint8Array;
}
export interface EngineFingerprint {
  schema: 1;
  sha256: string;
}
export interface Engine {
  checkpoint(): Promise<Checkpoint>;
  restoreCheckpoint(id: string): Promise<void>;
  undo(): Promise<Record<string, string>>;
  history(): Promise<ReplayLog>;
  replayHistory(bytes: Uint8Array): Promise<void>;
  fingerprint(): Promise<EngineFingerprint>;
  prepare(config: EngineConfig): void;
  saveCamera(): Promise<void>;
  cameraSaves(
    listener: (event: { savedAt?: number; revision?: number; error?: string }) => void,
  ): () => void;
  replay(steps: readonly ReplayStep[]): Promise<void>;
  metrics(listener: (metrics: CameraPerformance) => void): () => void;
  profile(limit?: number): Promise<Record<string, unknown>>;
  errors(listener: (fault: EngineFault) => void): () => void;
  rollback(seconds: number): Promise<Record<string, string>>;
  errorReport(submission?: Promise<ReportSubmission>): Promise<Blob>;
  boot(
    dat: ArrayBuffer,
    config: EngineConfig,
    progress: (message: string, percent?: number) => void,
    media?: BootMedia,
  ): Promise<Input[]>;
  /** Restart with these files merged into existing media; matching paths are replaced. */
  restartWithCardFiles(
    files: CardFile[],
    progress: (message: string, percent?: number) => void,
  ): Promise<void>;
  formatCard(progress: (message: string, percent?: number) => void): Promise<void>;
  insertCard(present: boolean): Promise<void>;
  powerCycle(
    inputs: BootInput[],
    progress: (message: string, percent?: number) => void,
  ): Promise<void>;
  cardState(): Promise<CardState>;
  cardTree(revision?: number): Promise<CardTree>;
  readCardFile(path: string, options?: CardReadOptions): Promise<Uint8Array>;
  /** Live presence and modeled support, independently reported by the emulator. */
  connections(listener: (rows: Connection[]) => void): () => void;
  /** Media changes, including busy transitions; refresh the tree after writes settle. */
  cardChanges(listener: (state: CardState) => void): () => void;
  frames(listener: (frame: Frame) => void): () => void;
  press(id: string, stage?: string): Promise<void>;
  release(id: string): Promise<void>;
  rotate(id: string, steps: number): Promise<void>;
  touch(id: string, phase: TouchPhase, x: number, y: number): Promise<void>;
  setPosition(id: string, position: string): Promise<void>;
  setScene(scene: Scene | null): Promise<void>;
  power(on: boolean): Promise<void>;
  powerChanges(listener: (on: boolean) => void): () => void;
  dispose(): void;
}

declare global {
  interface Window {
    cameraEngine?: Engine;
  }
}
