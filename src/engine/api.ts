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
export interface EngineConfig { bundle: string; inputs: string }
export interface Engine {
  boot(dat: ArrayBuffer, config: EngineConfig, progress: (message: string, percent: number) => void): Promise<Input[]>;
  frames(listener: (frame: Frame) => void): () => void;
  press(id: string, stage?: string, point?: { x: number; y: number }): Promise<void>;
  release(id: string): Promise<void>;
  setPosition(id: string, position: string | number): Promise<void>;
  setScene(scene: Scene): Promise<void>;
  power(on: boolean): Promise<void>;
  dispose(): void;
}
