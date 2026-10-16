import type { EngineConfig } from './api';

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
  | { type: 'boot'; dat: ArrayBuffer; bundle: string; config: EngineConfig }
  | { type: 'request'; id: number; action: string; options?: Record<string, unknown> }
  | { type: 'sensor_reset' }
  | { type: 'sensor'; rgb: ArrayBuffer; width: number; height: number; sequence?: number };

const worker = self as unknown as DedicatedWorkerGlobalScope;
let py: Pyodide | undefined;
let peakWasmMemoryBytes = 0;
function runtime() { if (!py) throw Error('Runtime is not loaded.'); return py; }
function runtimeMetrics() {
  const wasmMemoryBytes = py?._module.HEAPU8.buffer.byteLength || 0;
  peakWasmMemoryBytes = Math.max(peakWasmMemoryBytes, wasmMemoryBytes);
  return { wasmMemoryBytes, peakWasmMemoryBytes };
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
function currentState(): State { return JSON.parse(runtime().runPython('json.dumps(session.sample())')); }
function publish(state: State, id: number, includeFrame: boolean) {
  const frame = includeFrame ? currentFrame() : new ArrayBuffer(0);
  send('result', { id, state, frame, runtime: runtimeMetrics() }, frame.byteLength ? [frame] : []);
}
async function prepareRuntime(bundle: string) {
  if (py) return py;
  const { loadPyodide } = await import(/* @vite-ignore */ new URL('pyodide/pyodide.mjs', bundle).href) as {
    loadPyodide(options: { indexURL: string }): Promise<Pyodide>;
  };
  py = await loadPyodide({ indexURL: new URL('pyodide/', bundle).href });
  await py.loadPackage(['numpy', 'pillow']);
  py.unpackArchive(await (await fetch(new URL('sources.zip', bundle))).arrayBuffer(), 'zip', { extractDir: '/repo' });
  py.FS.writeFile('/repo/native/python/camera_native/_native.so',
    new Uint8Array(await (await fetch(new URL('_native.so', bundle))).arrayBuffer()), { canOwn: true });
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
async function boot(dat: ArrayBuffer, bundle: string, config: EngineConfig) {
  send('progress', { message: 'Loading camera runtime…', percent: 3 });
  const runtime = await prepareRuntime(bundle), protocol = config.boot;
  send('progress', { message: 'Checking firmware…', percent: 8 });
  const inputs = await fetch(new URL('firmware-inputs.zip', bundle), { cache: 'no-store' });
  if (!inputs.ok) throw Error('Camera storage inputs are unavailable.');
  runtime.unpackArchive(await inputs.arrayBuffer(), 'zip', { extractDir: '/repo' });
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
prepare = boot_config['prepare']
getattr(importlib.import_module(prepare['module']), prepare['function'])(**prepare['options'])
boot_module = importlib.import_module(boot_config['module'])
transport = importlib.import_module(boot_config['sessionModule'])
request = transport.request
use_analytic_scene = transport.use_analytic_scene
set_sensor_scene = transport.set_sensor_scene
session = None
gc.collect()
`);
  const now = new Date();
  const clock = Math.floor((Date.UTC(now.getFullYear(), now.getMonth(), now.getDate(), now.getHours(), now.getMinutes(), now.getSeconds()) - Date.UTC(2000, 0, 1)) / 1000);
  runtime.globals.set('clock_seconds_2000', clock);
  runtime.runPython('boot_module.start(clock_seconds_2000)');
  runtime.globals.set('boot_step', protocol.step);
  while (true) {
    const progress: { instructions: number; failure?: unknown } = JSON.parse(runtime.runPython('boot_module.advance(boot_step)'));
    send('progress', { message: `Booting camera · ${progress.instructions.toLocaleString()} / ${protocol.limit.toLocaleString()} instructions`, percent: 10 + Math.floor(80 * progress.instructions / protocol.limit) });
    if (progress.failure) throw Error(JSON.stringify(progress.failure));
    if (progress.instructions >= protocol.limit) break;
    await new Promise(resolve => setTimeout(resolve, 0));
  }
  runtime.runPython('session = boot_module.finish()\nuse_analytic_scene(session)');
  for (const [id, position] of Object.entries(config.initialPositions ?? {})) {
    runtime.globals.set('initial_position', JSON.stringify({ id, position }));
    runtime.runPython("request(session, 'set_position', initial_position)");
  }
  let state: State = {};
  for (let spent = 0; spent < protocol.readyLimit; spent += protocol.step) {
    runtime.runPython('session._advance(boot_step)'); state = currentState();
    if (state.failure || state.execution_error) throw Error(JSON.stringify(state.failure || state.execution_error));
    if (ready(state, protocol.readyWhen)) break;
    send('progress', { message: 'Preparing camera display…', percent: 92 });
    await new Promise(resolve => setTimeout(resolve, 0));
  }
  if (!ready(state, protocol.readyWhen)) throw Error('Camera display did not become ready within its instruction budget.');
  const frame = currentFrame();
  send('ready', { state, frame, runtime: runtimeMetrics() }, frame.byteLength ? [frame] : []);
}
worker.onmessage = async ({ data }: MessageEvent<Message>) => {
  try {
    if (data.type === 'boot') { await boot(data.dat, data.bundle, data.config); return; }
    const py = runtime();
    if (data.type === 'request') {
      py.globals.set('viewer_action', data.action); py.globals.set('viewer_options', JSON.stringify(data.options || {}));
      const state: State = JSON.parse(py.runPython('request(session, viewer_action, viewer_options)'));
      publish(state, data.id, data.action === 'state' || data.action === 'key'); return;
    }
    if (data.type === 'sensor_reset') { py.runPython('use_analytic_scene(session)'); send('sensor_ready'); return; }
    if (data.type === 'sensor') {
      const bytes = py.toPy(new Uint8Array(data.rgb));
      py.globals.set('sensor_rgb_buffer', bytes); py.globals.set('sensor_width', data.width); py.globals.set('sensor_height', data.height);
      try { py.runPython('set_sensor_scene(session, sensor_width, sensor_height, bytes(sensor_rgb_buffer))'); }
      finally { py.globals.delete('sensor_rgb_buffer'); bytes.destroy(); }
      send('sensor_ready', { sequence: data.sequence });
    }
  } catch (error) { send('error', { id: 'id' in data ? data.id : null, error: String(error) }); }
};
