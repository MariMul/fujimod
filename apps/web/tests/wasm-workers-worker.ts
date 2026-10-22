import { EmulatorSession, loadEmulatorModule, type SessionCommands } from 'fujimod/emulator';

interface Status {
  ticks: string;
  instructions: string;
  failed: boolean;
}
interface Pixels {
  width: number;
  height: number;
  sequence: string;
  sha256: string;
}
type Entry =
  | { kind: 'phase-start' | 'phase-end'; name: string; stage: string; status: Status }
  | { kind: 'run'; tick: string }
  | { kind: 'frame'; value: Pixels | null }
  | {
      kind: 'boundary';
      name: string;
      status: Status;
      fingerprint: SessionCommands['fingerprint']['result'];
    }
  | {
      kind: 'command' | 'observe';
      action: keyof SessionCommands;
      options: SessionCommands[keyof SessionCommands]['request'];
    }
  | { kind: 'scene'; width: number; height: number; file: string; sha256: string };
interface Photo {
  path: string;
  bytes: number;
  sha256: string;
}
interface Fixture {
  schema: 1;
  ticksPerSecond: number;
  module: string;
  wasm: string;
  dat: string;
  wasmSha256: string;
  config: unknown;
  plan: Entry[];
  photos: Photo[];
  identity: unknown;
}
export interface WorkerPhase {
  name: string;
  stage: string;
  guestSeconds: number;
  wallSeconds: number;
  speed: number;
  heapBytes: number;
}
export interface CompleteWorkerReceipt {
  complete: true;
  identityPassed: true;
  workerPreference: {
    initial: SessionCommands['device_workers']['result'];
    selected: SessionCommands['device_workers']['result'];
  };
  wasmSha256: string;
  sourceIdentity: unknown;
  phases: WorkerPhase[];
  boundaries: number;
  frames: number;
  photos: Photo[];
  realTimePassed: boolean;
  photographicQualification: 'separate';
  jit: unknown;
  error?: string;
}
export type WorkerReceipt =
  | CompleteWorkerReceipt
  | {
      complete: false;
      identityPassed: false;
      phases: WorkerPhase[];
      error: string;
      lastEntry: EntryMemory | null;
    };
interface EntryMemory {
  index: number;
  kind: Entry['kind'];
  heapBytes: number;
  action?: keyof SessionCommands;
}
export type WorkerMessage =
  | { type: 'frame'; width: number; height: number; rgba: ArrayBuffer }
  | { type: 'photo'; path: string; bytes: ArrayBuffer }
  | { type: 'phase'; phase: WorkerPhase }
  | { type: 'result'; result: WorkerReceipt };
interface OwnerPort {
  onmessage: ((event: MessageEvent<{ workers: 0 | 2 }>) => void) | null;
  postMessage(message: WorkerMessage, transfer?: Transferable[]): void;
}
const owner = self as unknown as OwnerPort;
function check(value: unknown, message: string): asserts value {
  if (!value) throw Error(message);
}
function same(actual: unknown, expected: unknown, label: string) {
  const stable = (value: unknown) =>
    JSON.stringify(value, (_key, item: unknown) =>
      item !== null && typeof item === 'object' && !Array.isArray(item)
        ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b)))
        : item,
    );
  check(
    stable(actual) === stable(expected),
    `${label}: ${stable(actual)} differs from ${stable(expected)}`,
  );
}
async function digest(bytes: Uint8Array) {
  return [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes.slice().buffer))]
    .map((n) => n.toString(16).padStart(2, '0'))
    .join('');
}
async function bytes(url: string) {
  const response = await fetch(url);
  check(response.ok, `Artifact unavailable: ${url}`);
  return new Uint8Array(await response.arrayBuffer());
}
owner.onmessage = async (event: MessageEvent<{ workers: 0 | 2 }>) => {
  let session: EmulatorSession | undefined;
  const phases: WorkerPhase[] = [];
  let lastEntry: EntryMemory | null = null;
  let trapped = false;
  try {
    check(event.data.workers === 0 || event.data.workers === 2, 'Worker preference must be 0 or 2');
    const response = await fetch('/__wasm-workers/plan.json');
    check(response.ok, 'Replay plan unavailable');
    const fixture = (await response.json()) as Fixture;
    check(fixture.schema === 1, 'Unsupported replay fixture');
    check(
      Number.isSafeInteger(fixture.ticksPerSecond) && fixture.ticksPerSecond > 0,
      'Invalid guest clock',
    );
    let actualWasmSha256 = '';
    const [dat, module] = await Promise.all([
      bytes(fixture.dat),
      loadEmulatorModule(
        fixture.module,
        fixture.wasm,
        { executionPolicy: 'interactive' },
        async (input, init) => {
          const response = await fetch(input, init);
          actualWasmSha256 = await digest(new Uint8Array(await response.clone().arrayBuffer()));
          return response;
        },
      ),
    ]);
    same(actualWasmSha256, fixture.wasmSha256, 'Wasm binary identity');
    const scenes = new Map<string, Uint8Array>();
    for (const entry of fixture.plan) {
      if (entry.kind === 'scene' && !scenes.has(entry.file)) {
        const rgb = await bytes(entry.file);
        same(await digest(rgb), entry.sha256, 'Scene identity');
        scenes.set(entry.file, rgb);
      }
    }
    session = new EmulatorSession(module);
    session.create(fixture.config, dat, 'session');
    const initial = session.command('device_workers', {}).result;
    const selected = session.command('device_workers', { count: event.data.workers }).result;
    same(
      selected,
      { count: event.data.workers, effective: event.data.workers },
      'Selected executor',
    );
    const status = (): Status => {
      const s = session!.status();
      return {
        ticks: s.ticks.toString(),
        instructions: s.instructions.toString(),
        failed: s.failed,
      };
    };
    let started = 0,
      prior!: Status,
      boundaries = 0,
      frames = 0;
    for (const [index, entry] of fixture.plan.entries()) {
      lastEntry = {
        index,
        kind: entry.kind,
        heapBytes: module.HEAPU8.byteLength,
        ...('action' in entry ? { action: entry.action } : {}),
      };
      if (['scene', 'boundary'].includes(entry.kind)) console.log(JSON.stringify(lastEntry));
      switch (entry.kind) {
        case 'phase-start':
          same(status(), entry.status, `${entry.name} start`);
          prior = status();
          started = performance.now();
          break;
        case 'phase-end': {
          const wallSeconds = (performance.now() - started) / 1000,
            after = status();
          same(after, entry.status, `${entry.name} end`);
          const guestSeconds =
            Number(BigInt(after.ticks) - BigInt(prior.ticks)) / fixture.ticksPerSecond;
          const phase = {
            name: entry.name,
            stage: entry.stage,
            guestSeconds,
            wallSeconds,
            speed: guestSeconds / wallSeconds,
            heapBytes: module.HEAPU8.byteLength,
          };
          phases.push(phase);
          owner.postMessage({ type: 'phase', phase } satisfies WorkerMessage);
          break;
        }
        case 'run':
          session.run(BigInt(entry.tick));
          break;
        case 'command':
        case 'observe':
          session.command(entry.action, entry.options);
          break;
        case 'scene':
          session.scene({ width: entry.width, height: entry.height, rgb: scenes.get(entry.file)! });
          break;
        case 'frame': {
          const f = session.frame(1);
          const rgba = f?.rgba.slice();
          const actual =
            f && rgba
              ? {
                  width: f.width,
                  height: f.height,
                  sequence: f.sequence.toString(),
                  sha256: await digest(rgba),
                }
              : null;
          same(actual, entry.value, `LCD observation ${frames}`);
          frames++;
          if (f && rgba)
            owner.postMessage(
              {
                type: 'frame',
                width: f.width,
                height: f.height,
                rgba: rgba.buffer,
              } satisfies WorkerMessage,
              [rgba.buffer],
            );
          break;
        }
        case 'boundary':
          same(
            session.command('fingerprint', {}).result,
            entry.fingerprint,
            `${entry.name} full Session`,
          );
          same(status(), entry.status, `${entry.name} status`);
          same(
            session.command('device_workers', {}).result,
            selected,
            'Stable executor preference',
          );
          boundaries++;
          break;
      }
    }
    const fingerprint = session.command('fingerprint', {}).result;
    for (const photo of fixture.photos) {
      const data = session.command('card_read', { path: photo.path, maxBytes: 32 << 20 }).bytes;
      same(data.length, photo.bytes, 'JPEG size');
      same(await digest(data), photo.sha256, 'JPEG identity');
      owner.postMessage(
        { type: 'photo', path: photo.path, bytes: data.buffer } satisfies WorkerMessage,
        [data.buffer],
      );
    }
    same(session.command('fingerprint', {}).result, fingerprint, 'Card reads are observational');
    owner.postMessage({
      type: 'result',
      result: {
        complete: true,
        identityPassed: true,
        workerPreference: { initial, selected },
        wasmSha256: fixture.wasmSha256,
        sourceIdentity: fixture.identity,
        phases,
        boundaries,
        frames,
        photos: fixture.photos,
        realTimePassed: phases.filter((p) => p.stage === 'warm').every((p) => p.speed >= 1),
        photographicQualification: 'separate',
        jit: module.tcgJitStats(),
      },
    } satisfies WorkerMessage);
  } catch (error) {
    trapped = error instanceof WebAssembly.RuntimeError;
    owner.postMessage({
      type: 'result',
      result: { complete: false, identityPassed: false, phases, error: String(error), lastEntry },
    } satisfies WorkerMessage);
  } finally {
    // A trapped Wasm call may leave an owner borrow live; terminate its worker
    // instead of reentering the failed instance during cleanup.
    if (!trapped) session?.dispose();
  }
};
