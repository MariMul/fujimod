import { RequestTimings } from './performance';
import { WorkerRecovery } from './recovery-worker';
import { EngineError, faultFrom, stateFault } from './fault.ts';
import type { EngineConfig, CardState } from './api';

interface PyBuffer { data: Uint8Array; release(): void }
interface PyProxy { getBuffer(kind: string): PyBuffer; destroy(): void }
interface Pyodide {
  _module: { HEAPU8: Uint8Array };
  loadPackage(packages: string[]): Promise<void>;
  unpackArchive(data: ArrayBuffer, format: string, options: { extractDir: string }): void;
  FS: { mkdirTree(path: string): void; writeFile(path: string, data: Uint8Array, options: { canOwn: boolean }): void };
  globals: { set(name: string, value: unknown): void; delete(name: string): void };
  runPython<T = string>(source: string): T;
  toPy(value: Uint8Array): PyProxy;
}
interface State {
  failure?: unknown; execution_error?: unknown;
  [key: string]: unknown;
}
type Message =
  | { type: 'boot'; dat: ArrayBuffer; bundle: string; config: EngineConfig; cardFiles: { path: string; bytes: ArrayBuffer }[] }
  | { type: 'restart'; id: number; initialPositions: Record<string, string>; cardFiles: { path: string; bytes: ArrayBuffer }[] }
  | { type: 'card'; id: number; action: 'state' | 'tree' | 'read'; path?: string; revision?: number; maxBytes?: number }
  | { type: 'request'; id: number; action: string; options?: Record<string, unknown> }
  | { type: 'profile'; id: number; limit: number }
  | { type: 'recovery_status' }
  | { type: 'rollback'; id: number; seconds: number }
  | { type: 'sensor_reset' }
  | { type: 'sensor'; rgb: ArrayBuffer; width: number; height: number; sequence?: number };

const worker = self as unknown as DedicatedWorkerGlobalScope;
let py: Pyodide | undefined;
const recovery = new WorkerRecovery();
let recoverySources: ArrayBuffer, recoveryNative: ArrayBuffer;
let stopped = false;
let peakWasmMemoryBytes = 0;
const requestTimings = new RequestTimings();
const bootStages: { name: string; seconds: number }[] = [];
let bootStarted = 0;
function bootStage(name: string) { bootStages.push({ name, seconds: (performance.now()-bootStarted)/1000 }); }
function configurePreview() {
  runtime().runPython("browser_stream = getattr(session.board, 'sensor_stream', None)\nif browser_stream is not None: browser_stream.set_fps(1)\ndel browser_stream");
}
let activeConfig: EngineConfig | undefined;
function runtime() { if (!py) throw Error('Runtime is not loaded.'); return py; }
function runtimeMetrics() {
  const wasmMemoryBytes = py?._module.HEAPU8.buffer.byteLength || 0;
  peakWasmMemoryBytes = Math.max(peakWasmMemoryBytes, wasmMemoryBytes);
  return { wasmMemoryBytes, peakWasmMemoryBytes, bootStages, requestTimings: requestTimings.actions };
}
function send(type: string, fields: Record<string, unknown> = {}, transfer: Transferable[] = []) {
  worker.postMessage({ type, ...fields }, transfer);
}
function currentFrame(): ArrayBuffer {
  const view = runtime().runPython<PyProxy>('memoryview(session.raw)');
  const buffer = view.getBuffer('u8');
  try { return Uint8Array.from(buffer.data).buffer; }
  finally { buffer.release(); view.destroy(); }
}
function cardState(): CardState { return JSON.parse(runtime().runPython('transport.card_state(session)')); }
function currentState(): State { return JSON.parse(runtime().runPython('json.dumps(session.sample())')); }
function publish(state: State, id: number, includeFrame: boolean) {
  const frame = includeFrame ? currentFrame() : new ArrayBuffer(0);
  send('result', { id, state, frame, card: cardState(), runtime: runtimeMetrics() }, frame.byteLength ? [frame] : []);
}
async function prepareRuntime(bundle: string) {
  if (py) return py;
  const assets = Promise.all(['sources.zip', '_native.so'].map(async name => {
    const response = await fetch(new URL(name, bundle));
    if (!response.ok) throw Error('Emulator bundle is unavailable.');
    return response.arrayBuffer();
  }));
  const { loadPyodide } = await import(/* @vite-ignore */ new URL('pyodide/pyodide.mjs', bundle).href) as {
    loadPyodide(options: { indexURL: string }): Promise<Pyodide>;
  };
  py = await loadPyodide({ indexURL: new URL('pyodide/', bundle).href });
  await py.loadPackage(['numpy', 'pillow']);
  [recoverySources, recoveryNative] = await assets;
  py.unpackArchive(recoverySources, 'zip', { extractDir: '/repo' });
  py.FS.writeFile('/repo/native/python/camera_native/_native.so',
    new Uint8Array(recoveryNative.slice(0)), { canOwn: true });
  py.runPython("import os,sys; os.chdir('/repo'); sys.path[:0]=['/repo/native/python','/repo','/repo/tools']");
  return py;
}
function ready(state: State, paths: string[]) {
  return paths.every(path => {
    let value: unknown = state;
    for (const key of path.split('.')) value = typeof value === 'object' && value !== null ? (value as Record<string, unknown>)[key] : undefined;
    return Array.isArray(value) ? value.length > 0 : Boolean(value);
  });
}
async function boot(dat: ArrayBuffer, bundle: string, config: EngineConfig, files: { path: string; bytes: ArrayBuffer }[]) {
  bootStarted = performance.now(); bootStages.length = 0;
  send('progress', { message: 'Loading camera runtime…', percent: 3 });
  const runtime = await prepareRuntime(bundle), protocol = config.boot;
  bootStage('runtime-loaded');
  send('progress', { message: 'Checking firmware…', percent: 8 });
  const inputs = await fetch(new URL('firmware-inputs.zip', bundle), { cache: 'no-store' });
  if (!inputs.ok) throw Error('Camera storage inputs are unavailable.');
  const inputBytes = await inputs.arrayBuffer();
  await recovery.identify(dat, recoverySources, recoveryNative, inputBytes, { protocol, positions: config.initialPositions }, files);
  runtime.unpackArchive(inputBytes, 'zip', { extractDir: '/repo' });
  runtime.FS.mkdirTree(protocol.datPath.slice(0, protocol.datPath.lastIndexOf('/')));
  runtime.FS.writeFile(protocol.datPath, new Uint8Array(dat), { canOwn: true });
  runtime.globals.set('boot_config_json', JSON.stringify(protocol));
  runtime.runPython(`
import gc, hashlib, importlib, json
from pathlib import Path
for name, expected in json.loads(Path('/repo/web-inputs.json').read_text()).items():
    if hashlib.sha256((Path('/repo') / name).read_bytes()).hexdigest() != expected:
        raise ValueError('Input SHA-256 differs: ' + name)
boot_config = json.loads(boot_config_json)
from tools.firmware_registry import by_sha256
with Path(boot_config['datPath']).open('rb') as dat_source:
    firmware_id = by_sha256(hashlib.file_digest(dat_source, 'sha256').hexdigest())['id']
prepare = boot_config['prepare']
boot_module = importlib.import_module(boot_config['module'])
transport = importlib.import_module(boot_config['sessionModule'])
request = transport.request
use_analytic_scene = transport.use_analytic_scene
set_sensor_scene = transport.set_sensor_scene
session = None
gc.collect()
`);
  bootStage('inputs-verified');
  activeConfig = config;
  return startSession(config, files, false);
}
async function startSession(config: EngineConfig, files: { path: string; bytes: ArrayBuffer }[], restarting: boolean) {
  const runtime = py!, protocol = config.boot;
  const now = new Date();
  const clock = Math.floor((Date.UTC(now.getFullYear(), now.getMonth(), now.getDate(), now.getHours(), now.getMinutes(), now.getSeconds()) - Date.UTC(2000, 0, 1)) / 1000);
  runtime.globals.set('clock_seconds_2000', clock);
  if (restarting) recovery.bypassPersistence();
  const resumed = await recovery.restore(runtime, restarting || config.restart === true || files.length > 0);
  if (!resumed) {
    runtime.runPython("getattr(importlib.import_module(prepare['module']), prepare['function'])(**prepare['options'])");
    runtime.FS.mkdirTree('/card-seed');
    const seeds = files.map((file, index) => {
      const staged = `/card-seed/${index}.bin`;
      runtime.FS.writeFile(staged, new Uint8Array(file.bytes), { canOwn: true });
      return { path: file.path, staged };
    });
    runtime.globals.set('card_seeds_json', JSON.stringify(seeds));
    runtime.globals.set('restart_card', restarting);
    runtime.runPython(`
  card_seeds = json.loads(card_seeds_json)
  card_files = [(item['path'], Path(item['staged']).read_bytes()) for item in card_seeds]
  card_revision = None
  if restart_card:
      card_files, card_revision = transport.files_for_card_restart(session, card_files)
  session = None
  browser_history = None
  boot_module.session = None
  boot_module.board = None
  gc.collect()
  boot_module.start(clock_seconds_2000, firmware_id, card_files=card_files, card_revision=card_revision)
  for item in card_seeds:
      Path(item['staged']).unlink()
  del card_files, card_seeds
  `);
    runtime.globals.set('boot_step', protocol.step);
    while (true) {
      const progress: { instructions: number; failure?: unknown } = JSON.parse(runtime.runPython('boot_module.advance(boot_step)'));
      send('progress', { message: `Booting camera · ${progress.instructions.toLocaleString()} / ${protocol.limit.toLocaleString()} instructions`, percent: 10 + Math.floor(80 * progress.instructions / protocol.limit) });
      if (progress.failure) throw new EngineError(faultFrom(progress.failure));
      if (progress.instructions >= protocol.limit) break;
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    runtime.runPython('session = boot_module.finish()\nuse_analytic_scene(session)');
    bootStage('cold-boot-complete');
  } else {
    runtime.runPython('boot_module.board = session.board');
    send('progress', { message: 'Resuming saved camera…', percent: 95 });
  }
  for (const [id, position] of Object.entries(config.initialPositions ?? {})) {
    runtime.globals.set('initial_position', JSON.stringify({ id, position }));
    runtime.runPython("request(session, 'set_position', initial_position)");
  }
  let state: State = {};
  for (let spent = 0; spent < protocol.readyLimit; spent += protocol.step) {
    if (!resumed) runtime.runPython('session._advance(boot_step)'); state = currentState();
    const fault = stateFault(state); if (fault) throw new EngineError(fault);
    if (ready(state, protocol.readyWhen)) break;
    send('progress', { message: 'Preparing camera display…', percent: 92 });
    await new Promise(resolve => setTimeout(resolve, 0));
  }
  if (!ready(state, protocol.readyWhen)) throw Error('Camera display did not become ready within its instruction budget.');
  configurePreview();
  recovery.start(runtime);
  bootStage('shooting-frame-ready');
  if (!resumed) await recovery.capture(runtime);
  const frame = currentFrame();
  send('cache', recovery.status(runtime));
  stopped = false;
  return { state, frame, card: cardState(), runtime: runtimeMetrics() };
}
worker.onmessage = async ({ data }: MessageEvent<Message>) => {
  try {
    if (data.type === 'boot') {
      const result = await boot(data.dat, data.bundle, data.config, data.cardFiles);
      send('ready', result, [result.frame]); return;
    }
    if (data.type === 'restart') {
      if (!activeConfig) throw Error('Emulator has not booted.');
      activeConfig = { ...activeConfig, initialPositions: { ...activeConfig.initialPositions, ...data.initialPositions } };
      const result = await startSession(activeConfig, data.cardFiles, true);
      stopped = false;
      send('restarted', { id: data.id, ...result }, [result.frame]); return;
    }
    const py = runtime();
    if (data.type === 'recovery_status') { send('cache', recovery.status(py)); return; }
    if (data.type === 'rollback') {
      recovery.rollback(py, data.seconds); stopped = false;
      publish(currentState(), data.id, true); return;
    }
    if (stopped && data.type !== 'card') throw new EngineError({ kind: 'exception', message: 'The camera is paused. Use Undo or Restart to continue.', details: {} });
    if (data.type === 'card') {
      py.globals.set('card_request_json', JSON.stringify({ revision: data.revision ?? null, path: data.path, max_bytes: data.maxBytes ?? 64 * 1024 * 1024 }));
      py.runPython('card_request = json.loads(card_request_json)\ncard_revision = card_request["revision"]');
      if (data.action === 'state') send('card_result', { id: data.id, card: cardState() });
      else if (data.action === 'tree') {
        const tree = JSON.parse(py.runPython('transport.card_tree(session, card_revision)'));
        send('card_result', { id: data.id, card: cardState(), tree });
      } else if (data.action === 'read') {
        const view = py.runPython<PyProxy>('memoryview(transport.read_card_file(session, card_request["path"], card_revision, card_request["max_bytes"]))');
        const buffer = view.getBuffer('u8');
        try {
          const bytes = Uint8Array.from(buffer.data).buffer;
          send('card_result', { id: data.id, card: cardState(), bytes }, [bytes]);
        } finally { buffer.release(); view.destroy(); }
      }
      return;
    }
    if (data.type === 'profile') {
      py.globals.set('browser_profile_json', JSON.stringify({ limit: data.limit }));
      const report: State = JSON.parse(py.runPython('from native.web.performance import profile\nprofile(session, json.loads(browser_profile_json)["limit"])'));
      const fault = stateFault(report); if (fault) throw new EngineError(fault);
      send('profile_result', { id: data.id, report, runtime: runtimeMetrics() }); return;
    }
    if (data.type === 'request') {
      py.globals.set('viewer_action', data.action); py.globals.set('viewer_options', JSON.stringify(data.options || {}));
      const state: State = requestTimings.measure(data.action, () => JSON.parse(py.runPython('request(session, viewer_action, viewer_options)')));
      py.runPython("boot_module.board = session.board\nboot_module.trim_diagnostics()");
      const fault = stateFault(state); if (fault) throw new EngineError(fault);
      if (await recovery.capture(py)) send('cache', recovery.status(py));
      publish(state, data.id, data.action === 'state' || data.action === 'key'); return;
    }
    if (data.type === 'sensor_reset') { py.runPython('use_analytic_scene(session)'); configurePreview(); send('sensor_ready'); return; }
    if (data.type === 'sensor') {
      const bytes = py.toPy(new Uint8Array(data.rgb));
      py.globals.set('sensor_rgb_buffer', bytes); py.globals.set('sensor_width', data.width); py.globals.set('sensor_height', data.height);
      try { py.runPython('set_sensor_scene(session, sensor_width, sensor_height, bytes(sensor_rgb_buffer))'); configurePreview(); }
      finally { py.globals.delete('sensor_rgb_buffer'); bytes.destroy(); }
      send('sensor_ready', { sequence: data.sequence });
    }
  } catch (error) {
    if (data.type === 'card') { send('error', { id: data.id, error: String(error) }); return; }
    stopped = true;
    send('error', { id: 'id' in data ? data.id : null, fault: faultFrom(error) });
  }
};
