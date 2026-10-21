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

export interface Checkpoint {
  id: string;
  tick: string;
  instructions: string[];
}

export interface EngineFingerprint {
  schema: 1;
  sha256: string;
}
