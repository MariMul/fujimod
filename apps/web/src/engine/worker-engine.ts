import {
  CameraPace,
  RequestTimings,
  type CameraPerformance,
} from "./performance.ts";
import {
  EngineError,
  faultFrom,
  stateFault,
  type EngineFault,
} from "./fault.ts";
import type {
  Engine,
  EngineConfig,
  Frame,
  Input,
  Scene,
  CardFile,
  BootMedia,
  CardState,
  CardTree,
  CardReadOptions,
  BootInput,
} from "./api";
import { digest } from "./checkpoint-store.js";
import { errorReportZip } from "./report.ts";

type State = {
  input_positions?: Record<string, string>;
  virtual_ticks: number;
  image_frames?: number;
  power?: { on: boolean };
  display?: { width: number; height: number };
  failure?: unknown;
  execution_error?: unknown;
};
type Reply = { state: State; frame?: ArrayBuffer; card: CardState };
type CardReply = { card: CardState; tree?: CardTree; bytes?: ArrayBuffer };

/** Adapter for the current Pyodide session worker. No device IDs or register bindings. */
export class WorkerEngine implements Engine {
  private worker: Worker;
  private onError: (error: Error) => void;
  private pending = new Map<
    number,
    { resolve: (reply: unknown) => void; reject: (error: Error) => void }
  >();
  private listeners = new Set<(frame: Frame) => void>();
  private cardListeners = new Set<(state: CardState) => void>();
  private latestCard?: CardState;
  private serial = 0;
  private sequence = 0;
  private state: State = { virtual_ticks: 0 };
  private alive = false;
  private disposed = false;
  private fault?: EngineFault;
  private errorListeners = new Set<(fault: EngineFault) => void>();
  private timer = 0;
  private wall = 0;
  private ticks = 0;
  private nextFrame = 0;
  private nextPixels = 0;
  private lastImageFrame = -1;
  private presentedImageFrames = 0;
  private queue: Promise<unknown> = Promise.resolve();
  private inputs: Input[] = [];
  private powerInput?: string;
  private touches = new Map<string, { x: number; y: number }>();
  private progressListener?: (message: string, percent: number) => void;
  private selectedPositions: Record<string, string> = {};
  private scene: Scene | null = null;
  private sceneInput = false;
  private pace = new CameraPace();
  private presentation = new RequestTimings();
  private metricListeners = new Set<(metrics: CameraPerformance) => void>();
  private latestRuntime: Record<string, unknown> = {};
  private nextMetrics = 0;
  private bootReject?: (error: Error) => void;
  private started = false;
  private bootMedia?: BootMedia;
  private inputTable?: { url: string; value: Promise<{inputs: any[]}> };
  private workerFailed = false;
  private reportIdentity: Record<string, unknown> = {};
  private rawErrors: unknown[] = [];
  private recentInputs: Record<string, unknown>[] = [];
  constructor(createWorker: () => Worker, onError: (error: Error) => void) {
    this.onError = onError;
    this.worker = createWorker();
    this.worker.onerror = (event) => {
      this.workerFailed = true;
      this.rawErrors.push({
        source: "worker-event",
        message: event.message,
        filename: event.filename,
        line: event.lineno,
        column: event.colno,
      });
      this.fail(Error(event.message || "Emulator worker failed."));
    };
  }
  private fail(error: Error) {
    const first = !this.fault;
    this.rawErrors.push({
      source: "engine",
      guest_ticks: this.state.virtual_ticks,
      fault: faultFrom(error),
    });
    this.rawErrors = this.rawErrors.slice(-32);
    this.fault ??= faultFrom(error);
    error = new EngineError(faultFrom(error));
    this.alive = false;
    clearTimeout(this.timer);
    this.bootReject?.(error);
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    if (!first) return;
    for (const listener of this.errorListeners) listener(this.fault);
    this.onError(error);
  }
  private loadInputs(config: EngineConfig) {
    const url = new URL(config.inputs, document.baseURI).href;
    if (this.inputTable?.url !== url) this.inputTable = {url, value: (async () => {
      const response = await fetch(url);
      if (!response.ok) throw Error("Emulator inputs are unavailable.");
      return response.json();
    })()};
    return this.inputTable.value;
  }
  prepare(config: EngineConfig) {
    void this.loadInputs(config).catch(() => {});
    this.worker.postMessage({ type: "prepare", bundle: new URL(config.bundle, document.baseURI).href });
  }
  async boot(
    dat: ArrayBuffer,
    config: EngineConfig,
    progress: (message: string, percent: number) => void,
    media?: BootMedia,
    initialScene: Scene | null = this.scene,
  ) {
    this.bootMedia = media;
    this.sceneInput = config.boot.sceneInput;
    this.scene = initialScene;
    this.prepare(config);
    const seeds = (media?.identities ?? Promise.resolve([])).then(identities => ({ identities, error: undefined }), error => ({ identities: undefined, error }));
    this.reportIdentity = {
      firmwareSha256: await digest(dat),
      bundleVersion: null,
      sourcesSha256: null,
      nativeSha256: null,
      bundle: config.bundle,
      protocol: config.boot,
    };
    performance.mark("camera-engine-dat-hashed");
    this.progressListener = progress;
    const table = await this.loadInputs(config);
    this.powerInput = table.inputs.find(
      (item: { id: string; binding?: { type: string } }) =>
        item.binding?.type === "session_power",
    )?.id;
    this.inputs = table.inputs.map(
      (item: {
        id: string;
        label: string;
        kind: string;
        binding: null | {
          positions?: { id: string; label: string }[];
          default_position?: string;
          relative?: boolean;
          stages?: { id: string; label: string }[];
        };
        modeled?: boolean;
      }) => ({
        id: item.id,
        label: item.label,
        kind: item.kind,
        supported: item.binding !== null && item.modeled !== false,
        positions: item.binding?.positions,
        initial: item.binding?.default_position,
        relative: item.binding?.relative,
        stages: item.binding?.stages,
      }),
    );
    performance.mark("camera-engine-inputs-ready");
    await new Promise<void>((resolve, reject) => {
      this.bootReject = reject;
      this.worker.onmessage = ({ data }) => {
        if (data.runtime) {
          this.latestRuntime = data.runtime;
          if (data.runtime.identity)
            this.reportIdentity = {
              ...this.reportIdentity,
              ...data.runtime.identity,
            };
        }
        if (data.identity)
          this.reportIdentity = { ...this.reportIdentity, ...data.identity };
        if (data.type === "card_files_needed") {
          void (this.bootMedia?.load() ?? Promise.resolve([])).then(seedFiles => {
            if (this.disposed) return;
            const files = seedFiles.map(file => ({path:file.path,bytes:file.bytes.slice().buffer}));
            this.worker.postMessage({type:"card_files",files},files.map(file=>file.bytes));
          }).catch(error => this.fail(error as Error));
          return;
        }
        if (data.type === "profile_result") {
          const pending = this.pending.get(data.id);
          this.pending.delete(data.id);
          pending?.resolve(data.report);
        }
        if (data.type === "progress")
          this.progressListener?.(data.message, data.percent);
        if (data.type === "error") {
          this.rawErrors.push({ source: "worker", ...data });
          this.rawErrors = this.rawErrors.slice(-32);
          const pending = this.pending.get(data.id);
          if (!data.fault && pending) {
            this.pending.delete(data.id);
            pending.reject(Error(data.error));
          } else
            this.fail(new EngineError(data.fault ?? faultFrom(data.error)));
        }
        if (data.card) this.observeCard(data.card);
        if (data.type === "card_result") {
          const pending = this.pending.get(data.id);
          this.pending.delete(data.id);
          pending?.resolve(data);
        }
        if (data.type === "error_report_result") {
          const pending = this.pending.get(data.id);
          this.pending.delete(data.id);
          pending?.resolve(data);
        }
        if (data.type === "sensor_ready") {
          const pending = this.pending.get(data.id);
          this.pending.delete(data.id);
          pending?.resolve(data);
        }
        if (data.type === "ready") {
          this.bootMedia = undefined;
          performance.mark("camera-worker-ready");
          this.bootReject = undefined;
          this.state = data.state;
          this.publish(data.frame);
          performance.mark("camera-first-frame-presented");
          this.alive = true;
          this.wall = performance.now();
          this.ticks = this.state.virtual_ticks;
          this.pace.reset(this.ticks);
          this.schedule();
          resolve();
        }
        if (data.type === "result" || data.type === "restarted") {
          const pending = this.pending.get(data.id);
          this.pending.delete(data.id);
          const fault = stateFault(data.state);
          if (fault) {
            const error = new EngineError(fault);
            pending?.reject(error);
            this.fail(error);
            return;
          }
          this.state = { ...this.state, ...data.state };
          pending?.resolve(data);
        }
      };
      const bundle = new URL(config.bundle, document.baseURI).href;
      void seeds.then(({ identities, error }) => {
        if (!identities) throw error;
        if (this.disposed) throw Error("Emulator closed.");
        this.started = true;
        performance.mark("camera-engine-sent");
        const scene = this.scenePayload();
        this.worker.postMessage(
          { type: "boot", dat, config, seedIdentity: identities, bundle, scene },
          scene ? [dat, scene.rgb] : [dat],
        );
      }).catch(reject);
    });
    return this.inputs;
  }
  private rpc<T>(
    type:
      | "request"
      | "card"
      | "rollback"
      | "restart"
      | "profile"
      | "error_report"
      | "sensor"
      | "sensor_reset",
    fields: Record<string, unknown>,
    transfer: Transferable[] = [],
    timeoutMs?: number,
  ): Promise<T> {
    if (this.disposed) return Promise.reject(Error("Emulator closed."));
    const id = ++this.serial;
    return new Promise((resolve, reject) => {
      const timer = timeoutMs
        ? setTimeout(() => {
            this.pending.delete(id);
            reject(Error("Error report worker timed out."));
          }, timeoutMs)
        : undefined;
      this.pending.set(id, {
        resolve: (reply) => {
          clearTimeout(timer);
          resolve(reply as T);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.worker.postMessage({ type, id, ...fields }, transfer);
    });
  }
  private request(
    action: string,
    options: Record<string, unknown> = {},
  ): Promise<Reply> {
    if (!this.alive) return Promise.reject(Error("Emulator is not ready."));
    return this.rpc<Reply>("request", { action, options });
  }
  private observeCard(card: CardState) {
    const previous = this.latestCard;
    this.latestCard = card;
    if (
      !previous ||
      previous.revision !== card.revision ||
      previous.present !== card.present ||
      previous.busy !== card.busy ||
      previous.supported !== card.supported
    ) {
      for (const listener of this.cardListeners) listener({ ...card });
    }
  }
  restartWithCardFiles(
    files: CardFile[],
    progress: (message: string, percent: number) => void,
  ) {
    return this.restartCard(files, progress, false, []);
  }
  formatCard(progress: (message: string, percent: number) => void) {
    return this.restartCard([], progress, true, []);
  }
  powerCycle(
    inputs: BootInput[],
    progress: (message: string, percent: number) => void,
  ) {
    return this.restartCard([], progress, false, inputs);
  }
  async insertCard(present: boolean) {
    this.requireCardReady();
    await this.rpc("card", { action: "present", present });
  }
  private restartCard(
    files: CardFile[],
    progress: (message: string, percent: number) => void,
    replaceCard: boolean,
    bootInputs: BootInput[],
  ) {
    const task = this.queue.then(async () => {
      if (!this.alive && !this.fault) throw Error("Emulator is not ready.");
      this.alive = false;
      clearTimeout(this.timer);
      const previousProgress = this.progressListener;
      this.progressListener = progress;
      const cardFiles = files.map((file) => ({
        path: file.path,
        bytes: file.bytes.slice().buffer,
      }));
      try {
        const scene = this.scenePayload();
        const reply = await this.rpc<Reply>(
          "restart",
          {
            cardFiles,
            initialPositions: this.selectedPositions,
            replaceCard,
            bootInputs,
            scene,
          },
          [...cardFiles.map((file) => file.bytes), ...(scene ? [scene.rgb] : [])],
        );
        this.state = reply.state;
        this.fault = undefined;
        this.publish(reply.frame);
        this.wall = performance.now();
        this.ticks = this.state.virtual_ticks;
        this.pace.reset(this.ticks);
        this.alive = true;
        this.nextFrame = this.nextPixels = 0;
        this.lastImageFrame = -1;
        await this.setScene(this.scene);
        this.schedule();
      } catch (error) {
        if (!this.fault && !this.disposed) {
          this.alive = true;
          this.schedule();
        }
        throw error;
      } finally {
        this.progressListener = previousProgress;
      }
    });
    this.queue = task.catch(() => {});
    return task;
  }
  private requireCardReady() {
    if (!this.alive && !this.fault) throw Error("Emulator is restarting.");
  }
  async cardState() {
    this.requireCardReady();
    return (await this.rpc<CardReply>("card", { action: "state" })).card;
  }
  async cardTree(revision?: number) {
    this.requireCardReady();
    return (await this.rpc<CardReply>("card", { action: "tree", revision }))
      .tree!;
  }
  async readCardFile(path: string, options: CardReadOptions = {}) {
    this.requireCardReady();
    const reply = await this.rpc<CardReply>("card", {
      action: "read",
      path,
      ...options,
    });
    return new Uint8Array(reply.bytes!);
  }
  cardChanges(listener: (state: CardState) => void) {
    this.cardListeners.add(listener);
    if (this.latestCard) listener({ ...this.latestCard });
    return () => {
      this.cardListeners.delete(listener);
    };
  }
  private publish(buffer?: ArrayBuffer) {
    if (!buffer?.byteLength || !this.state.display) return;
    const frame = {
      rgb: new Uint8Array(buffer),
      ...this.state.display,
      sequence: ++this.sequence,
    };
    this.presentation.measure("lcdCopy", () => {
      for (const listener of this.listeners) listener(frame);
    });
  }
  metrics(listener: (metrics: CameraPerformance) => void) {
    this.metricListeners.add(listener);
    return () => {
      this.metricListeners.delete(listener);
    };
  }
  profile(limit = 2_000_000) {
    const task = this.queue.then(async () => {
      if (!this.alive) throw Error("Emulator is not ready.");
      this.alive = false;
      clearTimeout(this.timer);
      try {
        return await this.rpc<Record<string, unknown>>("profile", { limit });
      } finally {
        if (!this.fault && !this.disposed) {
          this.alive = true;
          this.schedule();
        }
      }
    });
    this.queue = task.catch(() => {});
    return task;
  }
  errors(listener: (fault: EngineFault) => void) {
    this.errorListeners.add(listener);
    return () => {
      this.errorListeners.delete(listener);
    };
  }
  async errorReport(description: Promise<string> = Promise.resolve("")): Promise<Blob> {
    let detail: Record<string, unknown> & { checkpoint?: ArrayBuffer } = {};
    if (this.started && !this.workerFailed && !this.disposed) {
      try {
        detail = await this.rpc("error_report", {}, [], 30000);
      } catch (error) {
        detail = { snapshot: { unavailable: String(error) } };
      }
    } else
      detail = {
        snapshot: {
          unavailable:
            "The emulator worker is unavailable or has not initialized.",
        },
      };
    const { checkpoint, ...worker } = detail;
    const report = {
        schema: "fujimod-error-report-v1",
        createdAt: new Date().toISOString(),
        ...worker,
        error: this.fault ?? null,
        rawErrors: this.rawErrors,
        identity: {
          ...this.reportIdentity,
          ...((worker.identity as object) ?? {}),
        },
        clientInputs: this.recentInputs,
        state: this.state,
        runtime: this.latestRuntime,
        browser: {
          userAgent: navigator.userAgent,
          platform: navigator.platform,
          language: navigator.language,
        },
      };
    return errorReportZip({ ...report, description: await description }, checkpoint);
  }
  async rollback(seconds: number) {
    this.alive = false;
    clearTimeout(this.timer);
    const result = await this.rpc<Reply>("rollback", { seconds });
    this.state = result.state;
    this.fault = undefined;
    this.touches.clear();
    this.publish(result.frame);
    this.alive = true;
    this.wall = performance.now();
    this.ticks = this.state.virtual_ticks;
    this.pace.reset(this.ticks);
    this.nextFrame = this.nextPixels = 0;
    this.lastImageFrame = -1;
    this.schedule();
    const positions = result.state.input_positions ?? {};
    this.selectedPositions = { ...this.selectedPositions, ...positions };
    return positions;
  }
  frames(listener: (frame: Frame) => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private schedule() {
    this.timer = window.setTimeout(() => void this.pump(), 0);
  }
  private async pump() {
    if (!this.alive) return;
    try {
      const started = performance.now();
      do {
        const wallCap =
          this.ticks + Math.floor((performance.now() - this.wall) * 12000);
        const preview = this.latestRuntime.preview as {
          frames_per_second: number;
        } | null;
        const cap = preview ? Math.min(
          wallCap,
          this.state.virtual_ticks + Math.floor(6000000 / preview.frames_per_second),
        ) : wallCap;
        const before = this.state.virtual_ticks;
        await this.request("advance", { limit: 200000, pause_at_tick: cap });
        if (
          this.state.virtual_ticks === before ||
          this.state.image_frames !== this.lastImageFrame
        )
          break;
      } while (performance.now() - started < 12 && this.alive);
      if (!this.alive) return;
      if (
        this.state.image_frames !== this.lastImageFrame ||
        performance.now() >= this.nextFrame
      ) {
        const pixels =
          this.state.image_frames !== this.lastImageFrame ||
          performance.now() >= this.nextPixels;
        const sample = await this.request("state", { pixels });
        if (sample.frame?.byteLength) {
          if (this.state.image_frames !== this.lastImageFrame)
            this.presentedImageFrames++;
          this.publish(sample.frame);
          this.lastImageFrame = this.state.image_frames ?? -1;
          this.nextPixels = performance.now() + 500;
        }
        this.nextFrame = performance.now() + 100;
      }
      const metrics = this.pace.sample(this.state.virtual_ticks, {
        ...this.latestRuntime,
        presentation: this.presentation.actions,
        presentedImageFrames: this.presentedImageFrames,
      });
      if (performance.now() >= this.nextMetrics) {
        for (const listener of this.metricListeners) listener(metrics);
        this.nextMetrics = performance.now() + 250;
      }
      if (this.alive)
        this.timer = window.setTimeout(
          () => void this.pump(),
          metrics.virtualSeconds >= metrics.wallSeconds ? 8 : 0,
        );
    } catch (error) {
      this.fail(error as Error);
    }
  }
  private input(action: string, options: Record<string, unknown>) {
    this.recentInputs.push({
      action,
      options: { ...options },
      guest_ticks: this.state.virtual_ticks,
      timestampSource: "last-observed",
    });
    this.recentInputs = this.recentInputs.slice(-256);
    if (!this.alive)
      return Promise.reject(
        new EngineError(this.fault ?? faultFrom("Camera is paused.")),
      );
    // Serialize gestures, including quick pointerup before press acknowledgement.
    const task = this.queue.then(async () => {
      if (!this.alive)
        throw new EngineError(this.fault ?? faultFrom("Camera is paused."));
      await this.request(action, options);
    });
    this.queue = task.catch(() => {});
    return task;
  }
  press(id: string, stage?: string, point?: { x: number; y: number }) {
    const input = this.inputs.find((input) => input.id === id);
    if (input?.kind === "touch") {
      const pixels = {
        x: Math.round((point?.x ?? 0.5) * (this.state.display?.width ?? 720)),
        y: Math.round((point?.y ?? 0.5) * (this.state.display?.height ?? 480)),
      };
      this.touches.set(id, pixels);
      return this.input("touch", { id, phase: "press", ...pixels });
    }
    return this.input("press", { id, ...(stage ? { stage } : {}) });
  }
  release(id: string) {
    const touch =
      this.inputs.find((input) => input.id === id)?.kind === "touch";
    return this.input(
      touch ? "touch" : "release",
      touch ? { id, phase: "release", ...this.touches.get(id) } : { id },
    );
  }
  setPosition(id: string, position: string | number) {
    if (id === this.powerInput) return this.power(position === "on");
    const result =
      typeof position === "number"
        ? this.input("rotate", { id, steps: position })
        : this.input("set_position", { id, position });
    return result.then(() => {
      if (typeof position === "string") this.selectedPositions[id] = position;
    });
  }

  private scenePayload() {
    return this.scene ? { width: this.scene.width, height: this.scene.height,
      rgb: this.scene.rgb.slice().buffer } : undefined;
  }
  async setScene(scene: Scene | null) {
    if (!this.sceneInput) {
      if (scene) throw Error('This camera does not accept host scene images.');
      return;
    }
    this.scene = scene;
    if (!this.alive) return;
    if (!scene) {
      await this.rpc("sensor_reset", {});
      return;
    }
    const rgb = scene.rgb.slice().buffer;
    await this.rpc(
      "sensor",
      { rgb, width: scene.width, height: scene.height },
      [rgb],
    );
  }
  async power(on: boolean) {
    if (on) {
      if (this.state.power?.on !== false) return;
      return this.restartCard(
        [],
        this.progressListener ?? (() => {}),
        false,
        [],
      );
    }
    await this.input(on ? "power_on" : "power_off", {});
    this.wall = performance.now();
    this.ticks = this.state.virtual_ticks;
    this.pace.reset(this.ticks);
  }
  dispose() {
    this.bootMedia = undefined;
    this.disposed = true;
    this.alive = false;
    clearTimeout(this.timer);
    this.worker.terminate();
    const error = Error("Emulator closed.");
    this.bootReject?.(error);
    this.bootReject = undefined;
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    this.listeners.clear();
    this.cardListeners.clear();
    this.errorListeners.clear();
    this.metricListeners.clear();
  }
}
