import type {
  BootInput,
  BootMedia,
  CardFile,
  CardReadOptions,
  CardState,
  CardTree,
  Connection,
  Engine,
  EngineConfig,
  Frame,
  Input,
  ReplayStep,
  Scene,
} from './api';
import { loadImageScene } from './image-scene';
import { EngineError, faultFrom, type EngineFault } from './fault';
import type { CameraPerformance } from './performance';
import type { ReportSubmission } from './report';
import { zipFiles } from '../archive';
import type { WasmRequest, WasmWorkerEvent, WasmWorkerRequest } from './wasm-protocol';

type SaveEvent = { savedAt?: number; revision?: number; error?: string };
class Listeners<T> {
  private listeners = new Set<(value: T) => void>();
  add(listener: (value: T) => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  emit(value: T) {
    for (const listener of this.listeners) listener(value);
  }
}
/** Main-thread transport only; guest execution and pacing belong to the worker. */
export class WasmEngine implements Engine {
  private worker: Worker;
  private serial = 0;
  private pending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (error: Error) => void }
  >();
  private frameListeners = new Listeners<Frame>();
  private metricListeners = new Listeners<CameraPerformance>();
  private faultListeners = new Listeners<EngineFault>();
  private cardListeners = new Listeners<CardState>();
  private connectionListeners = new Listeners<Connection[]>();
  private saveListeners = new Listeners<SaveEvent>();
  private progress?: (message: string, percent?: number) => void;
  private fault?: EngineFault;
  private disposed = false;
  private unavailable?: Error;
  private selectedScene: Scene | null = null;
  private defaultScene?: Promise<Scene>;
  private sceneInput = false;
  private booted = false;
  private latestCard?: CardState;
  private latestConnections?: Connection[];
  constructor(
    private onError: (error: Error) => void,
    createWorker = () =>
      new Worker(new URL('./wasm-worker.ts', import.meta.url), { type: 'module' }),
  ) {
    this.worker = createWorker();
    this.worker.onmessage = (event: MessageEvent<WasmWorkerEvent>) => this.receive(event.data);
    this.worker.onerror = (event) => this.transportError(event.message || 'Wasm worker failed.');
    this.worker.onmessageerror = () =>
      this.transportError('Wasm worker message could not be decoded.');
  }
  private transportError(message: string) {
    this.unavailable = Error(message);
    this.fail(faultFrom(this.unavailable));
  }
  private fail(fault: EngineFault) {
    if (this.fault || this.disposed) return;
    this.fault = fault;
    const error = new EngineError(fault);
    for (const request of this.pending.values()) request.reject(error);
    this.pending.clear();
    this.faultListeners.emit(fault);
    this.onError(error);
  }
  private receive(event: WasmWorkerEvent) {
    if (this.disposed) {
      if (event.type === 'frame') event.bitmap.close();
      return;
    }
    switch (event.type) {
      case 'reply': {
        const p = this.pending.get(event.serial);
        this.pending.delete(event.serial);
        p?.resolve(event.result);
        return;
      }
      case 'error': {
        if (event.serial !== undefined) {
          const p = this.pending.get(event.serial);
          this.pending.delete(event.serial);
          p?.reject(event.fault ? new EngineError(event.fault) : Error(event.error));
        }
        if (event.fault) this.fail(event.fault);
        return;
      }
      case 'progress':
        this.progress?.(event.message, event.percent);
        return;
      case 'state':
        this.latestCard = event.state.card;
        this.latestConnections = event.state.connections;
        this.cardListeners.emit(event.state.card);
        this.connectionListeners.emit(event.state.connections);
        return;
      case 'metrics':
        this.metricListeners.emit(event.metrics);
        return;
      case 'saved':
        this.saveListeners.emit(event);
        return;
      case 'frame':
        try {
          this.frameListeners.emit({
            bitmap: event.bitmap,
            width: event.width,
            height: event.height,
            sequence: event.sequence,
          });
        } finally {
          event.bitmap.close();
        }
        return;
      case 'frame_view':
        try {
          this.frameListeners.emit({
            rgba: new Uint8ClampedArray(event.buffer, event.offset, event.length),
            width: event.width,
            height: event.height,
            sequence: event.sequence,
          });
        } finally {
          this.worker.postMessage({
            type: 'frame_release',
            lease: event.lease,
          } satisfies WasmWorkerRequest);
        }
    }
  }
  private rpc<T = void>(request: WasmRequest, transfer: Transferable[] = []): Promise<T> {
    if (this.disposed) return Promise.reject(Error('Emulator closed.'));
    if (this.unavailable) return Promise.reject(this.unavailable);
    const serial = ++this.serial;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(serial, { resolve: (value) => resolve(value as T), reject });
      try {
        this.worker.postMessage(
          { type: 'rpc', serial, request } satisfies WasmWorkerRequest,
          transfer,
        );
      } catch (error) {
        this.pending.delete(serial);
        reject(error);
      }
    });
  }
  private absolute(config: EngineConfig): EngineConfig {
    return {
      ...config,
      bundle: new URL(config.bundle, document.baseURI).href,
      inputs: new URL(config.inputs, document.baseURI).href,
    };
  }
  prepare(config: EngineConfig) {
    void this.rpc({ action: 'prepare', bundle: this.absolute(config).bundle }).catch(() => {});
  }
  async boot(
    dat: ArrayBuffer,
    config: EngineConfig,
    progress: (message: string, percent?: number) => void,
    media?: BootMedia,
  ): Promise<Input[]> {
    this.progress = progress;
    this.sceneInput = config.boot.sceneInput;
    progress('Loading Wasm runtime…');
    const [files, seedIdentity] = await Promise.all([
      media?.load() ?? Promise.resolve([]),
      media?.identities ?? Promise.resolve([]),
    ]);
    const scene = this.sceneInput
      ? (this.selectedScene ??
        (await (this.defaultScene ??= loadImageScene(
          new URL('./assets/default-dunes.png', import.meta.url),
        ))))
      : undefined;
    const inputs = await this.rpc<Input[]>(
      { action: 'boot', dat, config: this.absolute(config), files, scene, seedIdentity },
      [dat],
    );
    this.booted = true;
    return inputs;
  }
  frames(listener: (frame: Frame) => void) {
    return this.frameListeners.add(listener);
  }
  metrics(listener: (metrics: CameraPerformance) => void) {
    return this.metricListeners.add(listener);
  }
  errors(listener: (fault: EngineFault) => void) {
    if (this.fault) listener(this.fault);
    return this.faultListeners.add(listener);
  }
  connections(listener: (rows: Connection[]) => void) {
    if (this.latestConnections) listener(this.latestConnections);
    return this.connectionListeners.add(listener);
  }
  cardChanges(listener: (state: CardState) => void) {
    if (this.latestCard) listener(this.latestCard);
    return this.cardListeners.add(listener);
  }
  cameraSaves(listener: (event: SaveEvent) => void) {
    return this.saveListeners.add(listener);
  }
  saveCamera() {
    return this.rpc({ action: 'save' });
  }
  replay(steps: readonly ReplayStep[]) {
    return this.rpc({ action: 'replay', steps });
  }
  profile(limit?: number) {
    return this.rpc<Record<string, unknown>>({ action: 'profile', limit });
  }
  async rollback(seconds: number) {
    const positions = await this.rpc<Record<string, string>>({ action: 'rollback', seconds });
    this.fault = undefined;
    return positions;
  }
  async errorReport(submission?: Promise<ReportSubmission>) {
    const exported = await this.rpc<{ report: Record<string, unknown>; snapshot?: ArrayBuffer }>({
      action: 'error_report',
    }).catch((error: unknown) => ({ report: { unavailable: String(error) }, snapshot: undefined }));
    const files: [string, Uint8Array<ArrayBuffer>][] = [
      [
        'report.json',
        new TextEncoder().encode(
          JSON.stringify(
            { ...exported.report, fault: this.fault, submission: await submission },
            null,
            2,
          ),
        ),
      ],
    ];
    if (exported.snapshot) files.push(['snapshot.bin', new Uint8Array(exported.snapshot)]);
    return zipFiles(files);
  }

  async restartWithCardFiles(
    files: CardFile[],
    progress: (message: string, percent?: number) => void,
  ) {
    this.progress = progress;
    progress('Restarting camera…');
    await this.rpc({ action: 'restart_card', files });
  }
  async formatCard(progress: (message: string, percent?: number) => void) {
    this.progress = progress;
    progress('Formatting card…');
    await this.rpc({ action: 'card_format' });
  }
  insertCard(present: boolean) {
    return this.rpc({ action: 'card_insert', present });
  }
  async powerCycle(inputs: BootInput[], progress: (message: string, percent?: number) => void) {
    this.progress = progress;
    progress('Restarting camera…');
    await this.rpc({ action: 'power_cycle', inputs });
  }
  cardState() {
    return this.rpc<CardState>({ action: 'card_state' });
  }
  cardTree(revision?: number) {
    return this.rpc<CardTree>({ action: 'card_tree', revision });
  }
  readCardFile(path: string, options?: CardReadOptions) {
    return this.rpc<Uint8Array>({ action: 'card_read', path, options });
  }
  press(id: string, stage?: string, point?: { x: number; y: number }) {
    return this.rpc({ action: 'press', id, stage, point });
  }
  release(id: string) {
    return this.rpc({ action: 'release', id });
  }
  setPosition(id: string, position: string | number) {
    return this.rpc({ action: 'set_position', id, position });
  }
  async setScene(scene: Scene | null) {
    this.selectedScene = scene;
    if (!this.booted) return;
    const image = this.sceneInput
      ? (scene ??
        (await (this.defaultScene ??= loadImageScene(
          new URL('./assets/default-dunes.png', import.meta.url),
        ))))
      : scene;
    if (this.selectedScene !== scene) return;
    await this.rpc({ action: 'scene', scene: image });
  }
  power(on: boolean) {
    return this.rpc({ action: 'power', on });
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.worker.terminate();
    for (const request of this.pending.values()) request.reject(Error('Emulator closed.'));
    this.pending.clear();
  }
}
