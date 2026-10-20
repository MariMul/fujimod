import type { CardFile, EngineConfig, Input, ReplayStep, Scene } from './api';
import { CameraStore } from '../cameras/store';
import { digest } from './checkpoint-store';
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
}
export async function loadWasm(bundle: string): Promise<Loaded> {
  const response = await fetch(new URL('wasm.json', bundle));
  if (!response.ok) throw Error('Wasm runtime manifest is unavailable.');
  const manifest: WasmManifest = await response.json();
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
  const binaryResponse = await fetch(new URL(manifest.wasm, bundle));
  if (!binaryResponse.ok) throw Error('Wasm binary is unavailable.');
  const wasmBinary = await binaryResponse.arrayBuffer();
  const { default: create } = (await import(
    /* @vite-ignore */ new URL(manifest.module, bundle).href
  )) as {
    default: (options: {
      wasmBinary: ArrayBuffer;
      locateFile: (path: string) => string;
    }) => Promise<CameraModule>;
  };
  const module = await create({ wasmBinary, locateFile: (path) => new URL(path, bundle).href });
  return { session: new WasmSession(module), manifest, identity: await digest(wasmBinary) };
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
  private loaded?: Promise<Loaded>;
  private bundle?: string;
  private session?: WasmSession;
  private manifest?: WasmManifest;
  private config?: EngineConfig;
  private state?: SessionState;
  private queue: Promise<void> = Promise.resolve();
  private timer?: ReturnType<typeof setTimeout>;
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
  private sequence = -1n;
  private canvas?: OffscreenCanvas;
  private lease = 0;
  private release?: () => void;
  private leased?: Promise<void>;
  private runtimeKey = '';
  constructor(
    private send: Send,
    private loader = loadWasm,
    private cameras: Pick<CameraStore, 'saved' | 'save'> = new CameraStore(),
  ) {}
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
          if (action === 'press' && (request.stage || request.point))
            throw Error('Resolved key ABI does not support staged or touch input.');
          s.input(request.id, action === 'press');
        } else if (action === 'press') this.observe(s.command('press', options).result);
        else this.observe(s.command('release', options).result);
        return;
      }
      case 'set_position':
        this.observe(
          s.command('set_position', { id: request.id, position: request.position }).result,
        );
        return;
      case 'scene':
        s.scene(request.scene);
        return;
      case 'power':
        this.observe(s.command('power', { on: request.on }).result);
        this.rebase();
        this.publish();
        return;
      case 'power_cycle':
        this.observe(s.command('power_cycle', { inputs: request.inputs }).result);
        this.sequence = -1n;
        this.rebase();
        this.publish();
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
        this.observe(s.command('card_format', {}).result);
        this.rebase();
        this.sequence = -1n;
        this.publish();
        return;
      case 'restart_card': {
        const { rows, bytes } = packFiles(request.files);
        this.observe(s.command('restart_card', { files: rows }, bytes).result);
        this.rebase();
        this.sequence = -1n;
        this.publish();
        return;
      }
      case 'rollback': {
        if (!Number.isFinite(request.seconds) || request.seconds <= 0)
          throw Error('Invalid rollback duration.');
        this.observe(s.command('rollback', { seconds: request.seconds }).result);
        this.failed = false;
        this.sequence = -1n;
        this.rebase();
        this.publish();
        this.start();
        return this.state!.positions;
      }
      case 'profile':
        return s.command('profile', { limit: request.limit }).result;
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
        const { result, bytes } = s.command('error_report', {});
        return {
          report: { ...base, ...result.report },
          snapshot: result.snapshot ? extent(bytes, result.snapshot) : undefined,
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
    this.send({ type: 'progress', message: 'Loading Wasm runtime…' });
    const { session, manifest, identity } = await this.prepare(config.bundle);
    this.session = session;
    this.manifest = manifest;
    this.runtimeKey = await digest(
      new TextEncoder().encode(
        JSON.stringify([
          identity,
          manifest,
          config.firmwareId,
          await digest(dat),
          config.bootSource ?? 'internal',
          config.camera?.modEnabled ?? false,
          seedIdentity,
        ]),
      ),
    );
    this.send({ type: 'progress', message: 'Verifying firmware…' });
    session.create(
      manifest.construction === 'resolved'
        ? manifest.session
        : {
            schema: 1,
            graph: manifest.session,
            firmwareId: config.firmwareId,
            bootSource: config.bootSource ?? 'internal',
            initialPositions: config.initialPositions ?? {},
            clockSeconds2000: config.execution?.clockSeconds2000,
          },
      new Uint8Array(dat),
      manifest.construction,
    );
    if (manifest.construction === 'session') {
      const saved = config.camera && (await this.cameras.saved(config.camera.id));
      if (saved) {
        const snapshot =
          !config.restart && saved.runtimeKey === this.runtimeKey ? saved.snapshot : undefined;
        const bytes = new Uint8Array(saved.durable.byteLength + (snapshot?.byteLength ?? 0));
        bytes.set(new Uint8Array(saved.durable));
        if (snapshot) bytes.set(new Uint8Array(snapshot), saved.durable.byteLength);
        this.observe(
          session.command(
            'restore',
            {
              durable: { offset: 0, length: saved.durable.byteLength },
              positions: saved.positions,
              poweredOn: saved.poweredOn,
              snapshot: snapshot
                ? { offset: saved.durable.byteLength, length: snapshot.byteLength }
                : undefined,
            },
            bytes,
          ).result,
        );
        if (files.length && !snapshot) {
          const packed = packFiles(files);
          this.observe(
            session.command('restart_card', { files: packed.rows }, packed.bytes).result,
          );
        }
      } else if (files.length) {
        const packed = packFiles(files);
        this.observe(session.command('restart_card', { files: packed.rows }, packed.bytes).result);
      }
      this.observeCurrent();
    } else {
      if (files.length || config.camera || Object.keys(config.initialPositions ?? {}).length)
        throw Error('Resolved ABI does not support storage or initial positions.');
    }
    if (scene) session.scene(scene);
    const before = session.status().ticks;
    if (manifest.bootTicks && this.state?.poweredOn !== false)
      session.run(before + BigInt(manifest.bootTicks));
    this.observeCurrent();
    this.replayMode = config.execution?.mode === 'replay';
    this.started = this.metricWall = performance.now();
    this.startedTicks = this.metricTicks = session.status().ticks;
    this.rebase();
    let inputs: Input[];
    if (this.state) inputs = this.state.inputs;
    else {
      const response = await fetch(config.inputs);
      if (!response.ok) throw Error('Wasm input metadata is unavailable.');
      const table: { inputs: Input[] } = await response.json();
      inputs = table.inputs;
    }
    if (config.camera && !this.replayMode) await this.autoSave();
    this.publish();
    this.start();
    return inputs;
  }
  private rebase() {
    this.clockWall = performance.now();
    this.clockTicks = this.require().status().ticks;
  }
  private start() {
    if (!this.replayMode && !this.failed && !this.live) {
      this.live = true;
      this.schedule();
    }
  }
  private stop() {
    this.live = false;
    clearTimeout(this.timer);
  }
  private schedule() {
    if (!this.live) return;
    this.timer = setTimeout(
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
            const maximum = now + BigInt(Math.max(1, Math.floor(rate / 60)));
            if (target > now && this.state?.poweredOn !== false) {
              s.run(target < maximum ? target : maximum);
              this.observeCurrent();
              if (performance.now() >= this.nextSave) await this.autoSave();
              this.publish();
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
      16,
    );
  }
  private async replayStep(step: ReplayStep) {
    if (step.action === 'advance') {
      if (!Number.isSafeInteger(step.ticks) || step.ticks <= 0)
        throw Error('Invalid replay tick count.');
      // Yield between bounded slices so inputs, disposal and browser messages remain responsive.
      const s = this.require(),
        end = s.status().ticks + BigInt(step.ticks);
      const quantum = BigInt(Math.max(1, Math.floor(this.manifest!.ticksPerSecond / 60)));
      while (s.status().ticks < end) {
        const next = s.status().ticks + quantum;
        s.run(next < end ? next : end);
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      }
      this.observeCurrent();
      this.publish();
      // A following replay step must not mutate a shared borrowed frame.
      await this.leased;
    } else await this.dispatch(step);
  }
  private publish() {
    const s = this.require(),
      status = s.status(),
      wall = performance.now(),
      rate = this.manifest!.ticksPerSecond;
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
        },
      },
    });
    this.metricTicks = status.ticks;
    this.metricWall = wall;
    const frame = s.frame(this.manifest!.output);
    if (!frame || frame.sequence === this.sequence) return;
    this.sequence = frame.sequence;
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
      context.putImageData(new ImageData(view, width, height), 0, 0);
      const bitmap = this.canvas.transferToImageBitmap();
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
        snapshot: result.snapshot ? extent(bytes, result.snapshot) : undefined,
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
