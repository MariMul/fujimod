import type { Engine, EngineConfig, Frame, Input, Scene } from './api';

type State = { virtual_ticks: number; display?: { width: number; height: number }; failure?: unknown; execution_error?: unknown };
type Reply = { state: State; frame?: ArrayBuffer };

/** Adapter for the current Pyodide session worker. No device IDs or register bindings. */
export class WorkerEngine implements Engine {
  private worker: Worker;
  private pending = new Map<number, { resolve: (reply: Reply) => void; reject: (error: Error) => void }>();
  private listeners = new Set<(frame: Frame) => void>();
  private serial = 0;
  private sequence = 0;
  private state: State = { virtual_ticks: 0 };
  private alive = false;
  private timer = 0;
  private wall = 0;
  private ticks = 0;
  private nextFrame = 0;
  private queue: Promise<unknown> = Promise.resolve();
  private inputs: Input[] = [];
  private powerInput?: string;
  private touches = new Map<string, { x: number; y: number }>();
  private bootReject?: (error: Error) => void;
  constructor(createWorker: () => Worker, private onError: (error: Error) => void) {
    this.worker = createWorker();
    this.worker.onerror = event => this.fail(Error(event.message || 'Emulator worker failed.'));
  }
  private fail(error: Error) {
    this.alive = false;
    clearTimeout(this.timer);
    this.bootReject?.(error);
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    this.onError(error);
  }
  async boot(dat: ArrayBuffer, config: EngineConfig, progress: (message: string, percent: number) => void) {
    const response = await fetch(new URL(config.inputs, document.baseURI));
    if (!response.ok) throw Error('Emulator inputs are unavailable.');
    const table = await response.json();
    this.powerInput = table.inputs.find((item: { id: string; binding?: { type: string } }) => item.binding?.type === 'session_power')?.id;
    this.inputs = table.inputs.map((item: { id: string; label: string; kind: string; binding: null | { positions?: { id: string; label: string }[]; default_position?: string; relative?: boolean; stages?: { id: string; label: string }[] }; modeled?: boolean }) => ({
      id: item.id, label: item.label, kind: item.kind,
      supported: item.binding !== null && item.modeled !== false,
      positions: item.binding?.positions, initial: item.binding?.default_position,
      relative: item.binding?.relative, stages: item.binding?.stages,
    }));
    await new Promise<void>((resolve, reject) => {
      this.bootReject = reject;
      this.worker.onmessage = ({ data }) => {
        if (data.type === 'progress') progress(data.message, data.percent);
        if (data.type === 'error') {
          const error = Error(data.error);
          const pending = this.pending.get(data.id);
          if (pending) { this.pending.delete(data.id); pending.reject(error); }
          else this.fail(error);
        }
        if (data.type === 'ready') {
          this.bootReject = undefined;
          this.state = data.state;
          this.publish(data.frame);
          this.alive = true;
          this.wall = performance.now(); this.ticks = this.state.virtual_ticks;
          this.schedule(); resolve();
        }
        if (data.type === 'result') {
          const pending = this.pending.get(data.id);
          this.pending.delete(data.id);
          if (data.state.failure || data.state.execution_error) {
            const error = Error(JSON.stringify(data.state.failure || data.state.execution_error));
            pending?.reject(error); this.fail(error); return;
          }
          this.state = { ...this.state, ...data.state };
          pending?.resolve(data);
        }
      };
      this.worker.postMessage({ type: 'boot', dat, bundle: new URL(config.bundle, document.baseURI).href }, [dat]);
    });
    return this.inputs;
  }
  private request(action: string, options: Record<string, unknown> = {}): Promise<Reply> {
    const id = ++this.serial;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ type: 'request', id, action, options });
    });
  }
  private publish(buffer?: ArrayBuffer) {
    if (!buffer?.byteLength || !this.state.display) return;
    const frame = { rgb: new Uint8Array(buffer), ...this.state.display, sequence: ++this.sequence };
    for (const listener of this.listeners) listener(frame);
  }
  frames(listener: (frame: Frame) => void) { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; }
  private schedule() { this.timer = window.setTimeout(() => void this.pump(), 0); }
  private async pump() {
    if (!this.alive) return;
    try {
      const started = performance.now();
      do {
        const cap = this.ticks + Math.floor((performance.now() - this.wall) * 12000);
        const before = this.state.virtual_ticks;
        await this.request('advance', { limit: 200000, pause_at_tick: cap });
        if (this.state.virtual_ticks === before) break;
      } while (performance.now() - started < 12 && this.alive);
      if (performance.now() >= this.nextFrame) {
        const sample = await this.request('state'); this.publish(sample.frame);
        this.nextFrame = performance.now() + 100;
      }
      if (this.alive) this.timer = window.setTimeout(() => void this.pump(), 16);
    } catch (error) { this.fail(error as Error); }
  }
  private input(action: string, options: Record<string, unknown>) {
    // Serialize gestures, including quick pointerup before press acknowledgement.
    const task = this.queue.then(async () => { await this.request(action, options); });
    this.queue = task.catch(() => {});
    return task;
  }
  press(id: string, stage?: string, point?: { x: number; y: number }) {
    const input = this.inputs.find(input => input.id === id);
    if (input?.kind === 'touch') {
      const pixels = { x: Math.round((point?.x ?? 0.5) * (this.state.display?.width ?? 720)), y: Math.round((point?.y ?? 0.5) * (this.state.display?.height ?? 480)) };
      this.touches.set(id, pixels);
      return this.input('touch', { id, phase: 'press', ...pixels });
    }
    return this.input('press', { id, ...(stage ? { stage } : {}) });
  }
  release(id: string) {
    const touch = this.inputs.find(input => input.id === id)?.kind === 'touch';
    return this.input(touch ? 'touch' : 'release', touch ? { id, phase: 'release', ...this.touches.get(id) } : { id });
  }
  setPosition(id: string, position: string | number) {
    if (id === this.powerInput) return this.power(position === 'on');
    return typeof position === 'number'
      ? this.input('rotate', { id, steps: position })
      : this.input('set_position', { id, position });
  }
  async setScene(scene: Scene) {
    const rgb = scene.rgb.slice().buffer;
    this.worker.postMessage({ type: 'sensor', rgb, width: scene.width, height: scene.height }, [rgb]);
  }
  async power(on: boolean) {
    await this.input(on ? 'power_on' : 'power_off', {});
    this.wall = performance.now(); this.ticks = this.state.virtual_ticks;
  }
  dispose() {
    this.alive = false; clearTimeout(this.timer);
    this.worker.terminate();
    const error = Error('Emulator closed.');
    this.bootReject?.(error); this.bootReject = undefined;
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear(); this.listeners.clear();
  }
}
