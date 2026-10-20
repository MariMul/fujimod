import { HostTimings } from './host-timings';
import { HostTasks } from './host-task';
import type { CardFile, EngineConfig, ReplayStep, Scene, Checkpoint, BootInput } from './api';
import { CameraStore } from '../cameras/store';
import { digest } from './identity';
import { diagnosticReport } from './diagnostics';
import { faultFrom } from './fault';
import {
  GuestExecutionError,
  WasmSession,
  type CameraModule,
  type SessionState,
  type WasmManifest,
} from './wasm-abi';
import type { WasmRequest, WasmWorkerEvent, WasmWorkerRequest } from './wasm-protocol';

type Send = (event: WasmWorkerEvent, transfer?: Transferable[]) => void;
interface Loaded {
  session: WasmSession;
  manifest: WasmManifest;
  identity: string;
  loadTimings?: ReturnType<HostTimings['snapshot']>;
}
export async function loadWasm(bundle: string): Promise<Loaded> {
  const timing = new HostTimings();
  const response = await timing.measureAsync('manifestFetch', () =>
    fetch(new URL('wasm.json', bundle)),
  );
  if (!response.ok) throw Error('Wasm runtime manifest is unavailable.');
  const manifest: WasmManifest = await timing.measureAsync('manifestParse', () => response.json());
  if (
    manifest.schema !== 1 ||
    manifest.abi !== 1 ||
    !['resolved', 'session'].includes(manifest.construction) ||
    !Number.isSafeInteger(manifest.ticksPerSecond) ||
    manifest.ticksPerSecond <= 0 ||
    !Number.isSafeInteger(manifest.bootTicks) ||
    manifest.bootTicks < 0 ||
    !Number.isSafeInteger(manifest.output) ||
    manifest.output < 0 ||
    manifest.output > 0xffffffff
  )
    throw Error('Unsupported Wasm manifest.');
  if (
    manifest.observations &&
    (manifest.observations.length > 64 ||
      manifest.observations.some(
        (o) =>
          !/^[a-zA-Z][a-zA-Z0-9_]*$/.test(o.name) ||
          !/^[0-9]+$/.test(o.address) ||
          !Number.isSafeInteger(o.size) ||
          o.size < 1 ||
          o.size > 4096,
      ))
  )
    throw Error('Invalid observation declaration.');
  // Load the independent module text and binary concurrently.
  const [wasmBinary, { default: create }] = await Promise.all([
    timing.measureAsync('binaryFetch', async () => {
      const response = await fetch(new URL(manifest.wasm, bundle));
      if (!response.ok) throw Error('Wasm binary is unavailable.');
      return response.arrayBuffer();
    }),
    timing.measureAsync(
      'moduleImport',
      async () =>
        (await import(/* @vite-ignore */ new URL(manifest.module, bundle).href)) as {
          default: (options: {
            wasmBinary: ArrayBuffer;
            locateFile: (path: string) => string;
          }) => Promise<CameraModule>;
        },
    ),
  ]);
  const module = await timing.measureAsync('instantiate', () =>
    create({
      wasmBinary,
      locateFile: (path) => new URL(path, bundle).href,
    }),
  );
  const identity = await timing.measureAsync('binaryDigest', () => digest(wasmBinary));
  return { session: new WasmSession(module), manifest, identity, loadTimings: timing.snapshot() };
}
function packFiles(files: CardFile[]) {
  let offset = 0;
  const rows = files.map(({ path, bytes }) => {
    const row = { path, offset, length: bytes.length };
    offset += bytes.length;
    return row;
  });
  const bytes = new Uint8Array(offset);
  files.forEach((file, i) => bytes.set(file.bytes, rows[i]!.offset));
  return { rows, bytes };
}
function extent(bytes: Uint8Array, span: { offset: number; length: number }): ArrayBuffer {
  if (
    !Number.isSafeInteger(span.offset) ||
    !Number.isSafeInteger(span.length) ||
    span.offset < 0 ||
    span.length < 0 ||
    span.offset + span.length > bytes.length
  )
    throw Error('Invalid session save extent.');
  return bytes.slice(span.offset, span.offset + span.length).buffer;
}

/** One serialized owner. Rendering borrows Wasm memory before any next mutation. */
export class WasmWorkerRuntime {
  private timings = new HostTimings();
  private hostTasks = new HostTasks();
  private loadTimings?: Loaded['loadTimings'];
  private loaded?: Promise<Loaded>;
  private bundle?: string;
  private session?: WasmSession;
  private manifest?: WasmManifest;
  private config?: EngineConfig;
  private state?: SessionState;
  private scene?: Scene;
  private queue: Promise<void> = Promise.resolve();
  private cancelSchedule?: () => void;
  private live = false;
  private failed = false;
  private replayMode = false;
  private clockWall = 0;
  private clockTicks = 0n;
  private metricWall = 0;
  private metricTicks = 0n;
  private started = 0;
  private startedTicks = 0n;
  private nextSave = Infinity;
  private points: Checkpoint[] = [];
  private nextCheckpoint = 0n;
  private nextPresentation = 0n;
  private nextInspection = 0n;
  private sourceFrames?: number;
  private sourceTicks = 0n;
  private canvas?: OffscreenCanvas;
  private lease = 0;
  private release?: () => void;
  private leased?: Promise<void>;
  private runtimeKey = '';
  constructor(
    private send: Send,
    private loader = loadWasm,
    private cameras: Pick<CameraStore, 'saved' | 'save'> = new CameraStore(),
  ) {
    const transport = send;
    this.send = (event, transfer) =>
      this.timings.measure('postMessage', () => transport(event, transfer));
  }
  receive(message: WasmWorkerRequest) {
    if (message.type === 'frame_release') {
      if (message.lease === this.lease) {
        this.release?.();
        this.release = undefined;
        this.leased = undefined;
      }
      return;
    }
    this.enqueue(async () => {
      try {
        const result = await this.dispatch(message.request);
        this.send(
          { type: 'reply', serial: message.serial, result },
          result instanceof Uint8Array && result.buffer instanceof ArrayBuffer
            ? [result.buffer]
            : [],
        );
      } catch (error) {
        const fatal =
          message.request.action === 'boot' ||
          error instanceof GuestExecutionError ||
          error instanceof WebAssembly.RuntimeError;
        if (fatal) {
          this.failed = true;
          this.stop();
        }
        this.send({
          type: 'error',
          serial: message.serial,
          error: error instanceof Error ? error.message : String(error),
          fault: fatal ? faultFrom(error) : undefined,
        });
      }
    });
  }
  private enqueue(operation: () => Promise<void>) {
    this.queue = this.queue.then(async () => {
      await this.leased;
      await operation();
    });
  }
  private prepare(bundle: string) {
    if (this.bundle && this.bundle !== bundle)
      throw Error('A worker owns exactly one Wasm runtime.');
    this.bundle = bundle;
    return (this.loaded ??= this.loader(bundle));
  }
  private require() {
    if (!this.session) throw Error('Wasm session is not ready.');
    return this.session;
  }
  private observe(state: SessionState) {
    this.state = state;
    this.send({ type: 'state', state });
  }
  private observeCurrent() {
    if (this.manifest?.construction === 'session')
      this.observe(this.require().command('state', {}).result);
  }
  private async dispatch(request: WasmRequest): Promise<unknown> {
    if (request.action === 'prepare') {
      await this.prepare(request.bundle);
      return;
    }
    if (request.action === 'boot')
      return this.boot(
        request.dat,
        request.config,
        request.files,
        request.scene,
        request.seedIdentity,
      );
    const s = this.require();
    switch (request.action) {
      case 'press':
      case 'release': {
        const { action, ...options } = request;
        if (this.manifest?.construction === 'resolved') {
          if (action === 'press' && request.stage)
            throw Error('Resolved key ABI does not support staged or touch input.');
          s.input(request.id, action === 'press');
        } else if (action === 'press') this.observe(s.command('press', options).result);
        else this.observe(s.command('release', options).result);
        return;
      }
      case 'rotate':
        this.observe(s.command('rotate', { id: request.id, steps: request.steps }).result);
        return;
      case 'touch':
        this.observe(
          s.command('touch', { id: request.id, phase: request.phase, x: request.x, y: request.y })
            .result,
        );
        return;
      case 'set_position':
        this.observe(
          s.command('set_position', { id: request.id, position: request.position }).result,
        );
        return;
      case 'scene':
        this.scene = request.scene ?? undefined;
        s.scene(request.scene);
        return;
      case 'power':
        if (this.state?.poweredOn === request.on) return;
        await this.coldChange(() => this.observe(s.command('power', { on: request.on }).result));
        if (this.config?.camera) await this.save();
        return;
      case 'power_cycle':
        await this.coldChange(
          () => this.observe(s.command('power_cycle', { inputs: request.inputs }).result),
          request.inputs,
        );
        return;
      case 'card_state':
        return s.command('card_state', {}).result;
      case 'card_tree':
        return s.command('card_tree', { revision: request.revision }).result;
      case 'card_read':
        return s.command('card_read', { path: request.path, ...request.options }).bytes;
      case 'card_insert':
        this.observe(s.command('card_insert', { present: request.present }).result);
        return;
      case 'card_format':
        await this.coldChange(() => {
          this.observe(s.command('card_format', {}).result);
          if (this.state?.poweredOn)
            this.observe(
              s.command('power_cycle', { inputs: this.config?.bootInputs ?? [] }).result,
            );
        }, this.config?.bootInputs);
        return;
      case 'restart_card': {
        const { rows, bytes } = packFiles(request.files);
        await this.coldChange(
          () =>
            this.observe(
              s.command(
                'restart_card',
                { files: rows, inputs: this.config?.bootInputs ?? [] },
                bytes,
              ).result,
            ),
          this.config?.bootInputs,
        );
        return;
      }
      case 'checkpoint':
        return this.captureCheckpoint();
      case 'restore_checkpoint':
        s.command('restore', { id: request.id });
        this.afterRestore();
        return;
      case 'undo':
        s.command('undo', {});
        this.afterRestore();
        return this.state?.positions ?? {};
      case 'history': {
        const log = s.command('history', {});
        return { ...log.result, bytes: log.bytes };
      }
      case 'replay_history':
        s.command('replay', {}, new Uint8Array(request.bytes));
        this.afterRestore();
        return;
      case 'fingerprint':
        return s.command('fingerprint', {}).result;
      case 'rollback': {
        if (!Number.isFinite(request.seconds) || request.seconds <= 0)
          throw Error('Invalid rollback duration.');
        const now = s.status().ticks;
        const target = now - BigInt(Math.round(request.seconds * this.manifest!.ticksPerSecond));
        const retained =
          this.points.filter((point) => BigInt(point.tick) <= target).at(-1) ?? this.points[0];
        try {
          s.command('rollback', { tick: String(target < 0n ? 0n : target) });
        } catch (error) {
          if (!(error instanceof Error) || !/retained history/.test(error.message) || !retained)
            throw error;
          s.command('restore', { id: retained.id });
        }
        this.afterRestore();
        return this.state?.positions ?? {};
      }
      case 'profile':
        return {
          ...s.command('profile', { limit: request.limit }).result,
          checkpoints: this.points,
          wasmBytes: s.module.HEAPU8.byteLength,
          jit: s.module.tcgJitStats?.(),
          hostTimings: {
            abi: s.timings.snapshot(),
            worker: this.timings.snapshot(),
            load: this.loadTimings,
          },
          observations: Object.fromEntries(
            (this.manifest?.observations ?? []).map((o) => [
              o.name,
              [...s.command('memory_read', { address: o.address, size: o.size }).bytes],
            ]),
          ),
        };
      case 'error_report': {
        if (!s.created)
          return {
            report: {
              runtime: 'wasm',
              runtimeKey: this.runtimeKey,
              firmwareId: this.config?.firmwareId,
              created: false,
              status: null,
              frame: null,
            },
          };
        const status = s.status();
        const base = {
          runtime: 'wasm',
          runtimeKey: this.runtimeKey,
          ticks: String(status.ticks),
          instructions: String(status.instructions),
          failed: status.failed,
        };
        if (!s.module._camera_command) return { report: base };
        const { result } = s.command('error_report', {});
        return {
          report: await diagnosticReport({ ...base, ...result.report }),
        };
      }
      case 'save':
        return this.save();
      case 'replay': {
        if (!this.replayMode) throw Error('Replay requires replay execution mode.');
        for (const step of request.steps) await this.replayStep(step);
        return;
      }
    }
  }
  private async boot(
    dat: ArrayBuffer,
    config: EngineConfig,
    files: CardFile[],
    scene: Scene | undefined,
    seedIdentity: { path: string; sha256: string }[],
  ) {
    if (this.session) throw Error('Wasm worker already booted.');
    this.config = config;
    const saved = config.camera ? await this.cameras.saved(config.camera.id) : undefined;
    this.scene = scene;
    this.send({ type: 'progress', message: 'Loading Wasm runtime…' });
    const { session, manifest, identity, loadTimings } = await this.prepare(config.bundle);
    this.loadTimings = loadTimings;
    this.session = session;
    this.manifest = manifest;
    this.runtimeKey = await this.timings.measureAsync('runtimeIdentity', async () =>
      digest(
        new TextEncoder().encode(
          JSON.stringify([
            identity,
            manifest,
            config.firmwareId,
            await digest(dat),
            config.bootInputs ?? [],
            config.camera?.modEnabled ?? false,
            seedIdentity,
          ]),
        ),
      ),
    );
    this.send({ type: 'progress', message: 'Verifying firmware…' });
    this.timings.measure('create', () =>
      session.create(
        manifest.construction === 'resolved'
          ? manifest.session
          : {
              schema: 1,
              graph: manifest.session,
              firmwareId: config.firmwareId,
              bootSource: manifest.session.bootSource,
              bootInputs: config.bootInputs ?? [],
              initialPositions: config.initialPositions ?? {},
              clockSeconds2000: config.execution?.clockSeconds2000,
            },
        new Uint8Array(dat),
        manifest.construction,
      ),
    );
    if (manifest.construction === 'session') {
      if (saved)
        this.observe(session.command('restore_durable', {}, new Uint8Array(saved.durable)).result);
      if (files.length) {
        const packed = packFiles(files);
        this.observe(
          session.command(
            'restart_card',
            { files: packed.rows, inputs: config.bootInputs ?? [] },
            packed.bytes,
          ).result,
        );
      } else if (saved && (config.restart || config.bootInputs?.length))
        this.observe(session.command('power_cycle', { inputs: config.bootInputs ?? [] }).result);
      this.observeCurrent();
    } else if (files.length || config.camera || Object.keys(config.initialPositions ?? {}).length)
      throw Error('Resolved ABI has no storage or initial positions.');
    this.replayMode = config.execution?.mode === 'replay';
    await this.coldBoot(config.bootInputs);
    const inputs = (this.state?.inputs ?? manifest.inputs ?? []).map((input) => ({
      ...input,
      initial: this.state?.positions[input.id] ?? input.initial,
    }));
    if (config.camera && !this.replayMode) await this.autoSave();
    return inputs;
  }
  private async coldChange(change: () => void, held: BootInput[] = []) {
    const wasLive = this.live;
    this.stop();
    try {
      change();
      await this.coldBoot(held);
    } catch (error) {
      if (wasLive) {
        this.rebase();
        this.start();
      }
      throw error;
    }
  }
  private async coldBoot(held: BootInput[] = []) {
    const s = this.require(),
      rate = this.manifest!.ticksPerSecond;
    this.points = [];
    this.failed = false;
    this.nextInspection = 0n;
    this.nextPresentation = 0n;
    this.nextCheckpoint = 0n;
    if (this.scene) s.scene(this.scene);
    if (this.state?.poweredOn !== false) {
      if (this.manifest!.bootTicks) s.run(s.status().ticks + BigInt(this.manifest!.bootTicks));
      if (this.scene) s.scene(this.scene);
      if (this.manifest!.construction === 'session' && this.manifest!.session.preview)
        s.command('preview', {});
      for (const input of held)
        if (input.position === undefined) s.command('release', { id: input.id });
      if (held.some((input) => input.position === undefined)) {
        const end = s.status().ticks + BigInt(rate * 5);
        for (;;) {
          this.observeCurrent();
          const q = this.state?.inputQueue;
          if (!q || (!q.active && !q.pending)) break;
          if (s.status().ticks >= end)
            throw Error('Boot contacts did not release within five guest seconds.');
          s.run(s.status().ticks + BigInt(Math.max(1, Math.round(rate / 10))));
          await this.yieldToHost();
        }
      }
    }
    this.observeCurrent();
    this.started = this.metricWall = performance.now();
    this.startedTicks = this.metricTicks = s.status().ticks;
    this.rebase();
    if (this.manifest!.construction === 'session') this.captureCheckpoint();
    this.publish(true);
    this.start();
  }
  private rebase() {
    this.clockWall = performance.now();
    this.clockTicks = this.require().status().ticks;
  }
  private start() {
    if (!this.replayMode && !this.failed && !this.live && this.state?.poweredOn !== false) {
      this.live = true;
      this.schedule();
    }
  }
  private stop() {
    this.live = false;
    this.cancelSchedule?.();
    this.cancelSchedule = undefined;
  }
  private yieldToHost() {
    return this.timings.measureAsync(
      'yield',
      () => new Promise<void>((resolve) => this.hostTasks.schedule(resolve)),
    );
  }
  private schedule() {
    if (!this.live) return;
    this.cancelSchedule = this.hostTasks.schedule(
      () =>
        this.enqueue(async () => {
          if (!this.live) return;
          try {
            const s = this.require(),
              rate = this.manifest!.ticksPerSecond,
              now = s.status().ticks;
            const target =
              this.clockTicks +
              BigInt(Math.max(0, Math.floor(((performance.now() - this.clockWall) * rate) / 1000)));
            const maximum = now + BigInt(Math.max(1, Math.floor(rate / 10)));
            if (target > now && this.state?.poweredOn !== false) {
              // A whole presentation quantum avoids tiny history transactions at the pacing edge.
              s.run(maximum);
              if (
                this.manifest!.construction === 'session' &&
                s.status().ticks >= this.nextCheckpoint
              )
                this.captureCheckpoint();
              if (performance.now() >= this.nextSave) await this.autoSave();
              if (s.status().ticks >= this.nextPresentation) {
                this.observeCurrent();
                this.publish();
              }
            } else if (performance.now() >= this.nextSave) await this.autoSave();
          } catch (error) {
            this.failed = true;
            this.stop();
            this.send({
              type: 'error',
              error: error instanceof Error ? error.message : String(error),
              fault: faultFrom(error),
            });
          }
          this.schedule();
        }),
      Math.max(
        0,
        Math.min(
          16,
          (Number(this.require().status().ticks - this.clockTicks) * 1000) /
            this.manifest!.ticksPerSecond -
            (performance.now() - this.clockWall),
        ),
      ),
    );
  }
  private captureCheckpoint(): Checkpoint {
    const { retainedIds, ...saved } = this.require().command('checkpoint', {}).result;
    this.points = this.points.filter((point) => retainedIds.includes(point.id));
    this.points.push(saved);
    this.points = this.points.slice(-8);
    this.nextCheckpoint = BigInt(saved.tick) + BigInt(this.manifest!.ticksPerSecond * 2);
    return saved;
  }
  private afterRestore() {
    const tick = this.require().status().ticks;
    this.points = this.points.filter((point) => BigInt(point.tick) <= tick);
    this.nextCheckpoint = tick + BigInt(this.manifest!.ticksPerSecond * 2);
    this.started = this.metricWall = performance.now();
    this.startedTicks = this.metricTicks = tick;
    this.nextInspection = tick;
    this.nextPresentation = tick;
    this.failed = this.require().status().failed;
    this.observeCurrent();
    this.rebase();
    this.publish(true);
    this.start();
  }
  private inspectSource(tick: bigint) {
    if (this.manifest!.construction !== 'session' || tick < this.nextInspection) return;
    this.sourceFrames = this.manifest!.session.preview
      ? this.require().command('preview_state', {}).result.state.frames
      : undefined;
    this.sourceTicks = tick;
    this.nextInspection = tick + BigInt(Math.round(this.manifest!.ticksPerSecond / 10));
  }
  private async replayStep(step: ReplayStep) {
    if (step.action === 'advance') {
      if (!Number.isSafeInteger(step.ticks) || step.ticks <= 0)
        throw Error('Invalid replay tick count.');
      if (this.state?.poweredOn === false) return;
      // Yield between bounded slices so inputs, disposal and browser messages remain responsive.
      const s = this.require(),
        end = s.status().ticks + BigInt(step.ticks);
      const quantum = BigInt(Math.max(1, Math.floor(this.manifest!.ticksPerSecond / 10)));
      while (s.status().ticks < end) {
        const next = s.status().ticks + quantum;
        s.run(next < end ? next : end);
        if (this.manifest!.construction === 'session' && s.status().ticks >= this.nextCheckpoint)
          this.captureCheckpoint();
        if (s.status().ticks >= this.nextPresentation) {
          this.observeCurrent();
          this.publish();
          await this.leased;
        }
        await this.yieldToHost();
      }
      this.observeCurrent();
      this.publish();
      // A following replay step must not mutate a shared borrowed frame.
      await this.leased;
    } else await this.dispatch(step);
  }
  private publish(force = false) {
    const s = this.require(),
      status = s.status(),
      wall = performance.now(),
      rate = this.manifest!.ticksPerSecond;
    if (!force && status.ticks < this.nextPresentation) return;
    this.inspectSource(status.ticks);
    this.nextPresentation = status.ticks + BigInt(Math.round(rate / 10));
    const elapsed = (wall - this.started) / 1000;
    this.send({
      type: 'metrics',
      metrics: {
        guestTicks: Number(status.ticks),
        virtualSeconds: Number(status.ticks - this.startedTicks) / rate,
        wallSeconds: elapsed,
        speed:
          wall > this.metricWall
            ? Number(status.ticks - this.metricTicks) / rate / ((wall - this.metricWall) / 1000)
            : 0,
        averageSpeed: elapsed > 0 ? Number(status.ticks - this.startedTicks) / rate / elapsed : 0,
        runtime: {
          instructions: String(status.instructions),
          ticksPerSecond: rate,
          runtimeKey: this.runtimeKey,
          sourceFrames: this.sourceFrames,
          sourceTicks: String(this.sourceTicks),
          checkpoints: this.points.map((point) => point.tick),
        },
      },
    });
    this.metricTicks = status.ticks;
    this.metricWall = wall;
    const frame = s.frame(this.manifest!.output);
    if (!frame) return;
    const { rgba, width, height } = frame,
      sequence = Number(frame.sequence);
    if (typeof SharedArrayBuffer !== 'undefined' && rgba.buffer instanceof SharedArrayBuffer) {
      this.leased = new Promise<void>((resolve) => {
        this.release = resolve;
      });
      this.send({
        type: 'frame_view',
        buffer: rgba.buffer,
        offset: rgba.byteOffset,
        length: rgba.length,
        width,
        height,
        sequence,
        lease: ++this.lease,
      });
    } else {
      if (!this.canvas || this.canvas.width !== width || this.canvas.height !== height)
        this.canvas = new OffscreenCanvas(width, height);
      const context = this.canvas.getContext('2d');
      if (!context) throw Error('Worker canvas is unavailable.');
      // Same backing memory: no slice, RGB conversion, PNG or pixel message clone.
      const view = new Uint8ClampedArray(
        rgba.buffer as ArrayBuffer,
        rgba.byteOffset,
        rgba.byteLength,
      );
      const bitmap = this.timings.measure('bitmap', () => {
        context.putImageData(new ImageData(view, width, height), 0, 0);
        return this.canvas!.transferToImageBitmap();
      });
      this.send({ type: 'frame', bitmap, width, height, sequence }, [bitmap]);
    }
  }
  private async autoSave() {
    try {
      await this.save();
      this.nextSave = performance.now() + 30000;
    } catch {
      this.nextSave = performance.now() + 10000;
    }
  }
  private async save() {
    const camera = this.config?.camera;
    try {
      if (!camera) throw Error('This session does not belong to a saved camera.');
      if (this.failed) throw Error('The stopped camera cannot replace its last good save.');
      const { result, bytes } = this.require().command('save', {});
      const record = await this.cameras.save(camera.id, camera.revision, {
        durable: extent(bytes, result.durable),
        runtimeKey: this.runtimeKey,
        poweredOn: result.poweredOn,
        positions: result.positions,
        modEnabled: camera.modEnabled,
      });
      camera.revision = record.revision;
      this.nextSave = performance.now() + 30000;
      this.send({ type: 'saved', savedAt: record.savedAt, revision: record.revision });
    } catch (error) {
      this.send({ type: 'saved', error: String(error) });
      throw error;
    }
  }
}
