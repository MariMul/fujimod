import sourceArchiveLoader from "./source-archive.py?raw";
import { cachedFetch } from "../asset-cache";
import { boundedMemoryModule, type RuntimeSettings } from "./wasm-growth";
import { RequestTimings } from "./performance";
import { WorkerRecovery } from "./recovery-worker";
import { digest } from "./checkpoint-store.js";
import { EngineError, FAULT_UNDO_SECONDS, faultFrom, stateFault } from "./fault.ts";
import type { EngineConfig, CardState } from "./api";

interface PyBuffer {
  data: Uint8Array;
  release(): void;
}
interface PyProxy {
  getBuffer(kind: string): PyBuffer;
  destroy(): void;
}
interface Pyodide {
  _module: { HEAPU8: Uint8Array };
  loadPackage(packages: string[]): Promise<void>;
  unpackArchive(
    data: ArrayBuffer,
    format: string,
    options: { extractDir: string },
  ): void;
  FS: {
    mkdirTree(path: string): void;
    writeFile(
      path: string,
      data: Uint8Array,
      options: { canOwn: boolean },
    ): void;
  };
  globals: {
    set(name: string, value: unknown): void;
    delete(name: string): void;
  };
  runPython<T = string>(source: string): T;
  toPy(value: Uint8Array): PyProxy;
}
interface State {
  failure?: unknown;
  execution_error?: unknown;
  [key: string]: unknown;
}
type SceneInput = { width: number; height: number; rgb: ArrayBuffer };
type Message =
  | { type: "prepare"; bundle: string }
  | {
      type: "boot";
      dat: ArrayBuffer;
      bundle: string;
      config: EngineConfig;
      seedIdentity: { path: string; sha256: string }[];
      scene?: SceneInput;
    }
  | { type: "card_files"; files: {path:string;bytes:ArrayBuffer}[] }
  | {
      type: "restart";
      id: number;
      initialPositions: Record<string, string>;
      cardFiles: { path: string; bytes: ArrayBuffer }[];
      replaceCard?: boolean;
      bootInputs?: { id: string; stage?: string; position?: string }[];
      scene?: SceneInput;
    }
  | {
      type: "card";
      id: number;
      action: "state" | "tree" | "read" | "present";
      present?: boolean;
      path?: string;
      revision?: number;
      maxBytes?: number;
    }
  | {
      type: "request";
      id: number;
      action: string;
      options?: Record<string, unknown>;
    }
  | { type: "profile"; id: number; limit: number }
  | { type: "recovery_status" }
  | { type: "error_report"; id: number }
  | { type: "rollback"; id: number; seconds: number }
  | { type: "sensor_reset"; id: number }
  | {
      type: "sensor";
      id: number;
      rgb: ArrayBuffer;
      width: number;
      height: number;
      sequence?: number;
    };

const worker = self as unknown as DedicatedWorkerGlobalScope;
let py: Pyodide | undefined;
let preparation: Promise<Pyodide> | undefined;
let seedIdentity: {path:string;sha256:string}[] = [];
let provideFiles: ((files: {path:string;bytes:ArrayBuffer}[]) => void) | undefined;
async function initialFiles() {
  const pending = new Promise<{path:string;bytes:ArrayBuffer}[]>(resolve=>{provideFiles=resolve;});
  send("card_files_needed");
  const files = await pending;
  if (JSON.stringify(await Promise.all(files.map(async file=>[file.path,await digest(file.bytes)]))) !== JSON.stringify(seedIdentity.map(file=>[file.path,file.sha256]))) throw Error("Initial card bytes differ from the declared seed identity.");
  return files;
}
const recovery = new WorkerRecovery();
let recoverySources: ArrayBuffer, recoveryNative: ArrayBuffer;
let stopped = false;
let peakWasmMemoryBytes = 0;
const requestTimings = new RequestTimings();
const bootStages: { name: string; seconds: number }[] = [];
let bootStarted = 0;
let inputSerial = 0;
let recentInputs: Record<string, unknown>[] = [];
let errorTrigger: Record<string, unknown> | undefined;
let previewPolicy: Record<string, unknown> = {};
function recordInput(action: string, options: Record<string, unknown> = {}) {
  const ticks = py?.runPython<number>(
    "session.board.virtual_ticks if globals().get('session') is not None else None",
  );
  recentInputs.push({
    sequence: ++inputSerial,
    action,
    options,
    guest_ticks: ticks ?? null,
    timestampSource: "guest-boundary",
  });
  recentInputs = recentInputs.slice(-256);
}
function bootStage(name: string) {
  bootStages.push({ name, seconds: (performance.now() - bootStarted) / 1000 });
}
function configurePreview() {
  previewPolicy = JSON.parse(runtime().runPython("transport.preview(session)"));
}
let activeConfig: EngineConfig | undefined;
function runtime() {
  if (!py) throw Error("Runtime is not loaded.");
  return py;
}
function runtimeMetrics() {
  const wasmMemoryBytes = py?._module.HEAPU8.buffer.byteLength || 0;
  peakWasmMemoryBytes = Math.max(peakWasmMemoryBytes, wasmMemoryBytes);
  return {
    wasmMemoryBytes,
    peakWasmMemoryBytes,
    bootStages,
    requestTimings: requestTimings.actions,
    identity: recovery.identity,
    preview: previewPolicy,
  };
}
function send(
  type: string,
  fields: Record<string, unknown> = {},
  transfer: Transferable[] = [],
) {
  worker.postMessage({ type, ...fields }, transfer);
}
function currentFrame(): ArrayBuffer {
  const view = runtime().runPython<PyProxy>("memoryview(session.raw)");
  const buffer = view.getBuffer("u8");
  try {
    return Uint8Array.from(buffer.data).buffer;
  } finally {
    buffer.release();
    view.destroy();
  }
}
function cardState(): CardState {
  return JSON.parse(runtime().runPython("transport.card_state(session)"));
}
function currentState(): State {
  return JSON.parse(runtime().runPython("transport.request(session, 'state')"));
}
function installScene(scene: SceneInput) {
  const py = runtime();
  const bytes = requestTimings.measure("sensor-transfer", () => py.toPy(new Uint8Array(scene.rgb)));
  py.globals.set("sensor_rgb_buffer", bytes);
  py.globals.set("sensor_geometry_json", JSON.stringify({ width: scene.width, height: scene.height }));
  try {
    requestTimings.measure("sensor-install", () => py.runPython(
      'sensor_geometry=json.loads(sensor_geometry_json)\nset_sensor_scene(session,sensor_geometry["width"],sensor_geometry["height"],bytes(sensor_rgb_buffer))',
    ));
  } finally {
    py.globals.delete("sensor_rgb_buffer");
    py.globals.delete("sensor_geometry_json");
    bytes.destroy();
  }
}
function publish(state: State, id: number, includeFrame: boolean) {
  const frame = includeFrame ? currentFrame() : new ArrayBuffer(0);
  send(
    "result",
    { id, state, frame, card: cardState(), runtime: runtimeMetrics() },
    frame.byteLength ? [frame] : [],
  );
}
function prepareRuntime(bundle: string) {
  return preparation ??= loadRuntime(bundle);
}
async function loadRuntime(bundle: string) {
  if (py) return py;
  const originalFetch = self.fetch.bind(self);
  const runtimeRoot = new URL(bundle).href;
  self.fetch = (input, init) => {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
    const method = init?.method || (input instanceof Request ? input.method : 'GET');
    return method === 'GET' && url.startsWith(runtimeRoot)
      ? cachedFetch(url, originalFetch) : originalFetch(input, init);
  };
  try {
    const assets = Promise.all(
      ["sources.zip", "_native.so"].map(async (name) => {
        const response = await fetch(new URL(name, bundle));
        if (!response.ok) throw Error("Emulator bundle is unavailable.");
        return response.arrayBuffer();
      }),
    );
    const { loadPyodide } = (await import(
      /* @vite-ignore */ new URL("pyodide/pyodide.mjs", bundle).href
    )) as {
      loadPyodide(options: { indexURL: string; packages: string[]; createPyodideModule: unknown }): Promise<Pyodide>;
    };
    py = await loadPyodide({ indexURL: new URL("pyodide/", bundle).href,
      packages: ["numpy", "pillow"],
      createPyodideModule: async (settings: RuntimeSettings) => {
        const {default: factory} = await import(/* @vite-ignore */ new URL("pyodide/pyodide.asm.mjs", bundle).href);
        return boundedMemoryModule(factory)(settings);
      },
    });
    [recoverySources, recoveryNative] = await assets;
    py.FS.mkdirTree("/repo/native/python/camera_native");
    py.FS.writeFile("/repo/runtime-sources.zip", new Uint8Array(recoverySources), {canOwn:true});
    py.FS.writeFile(
      "/repo/native/python/camera_native/_native.so",
      new Uint8Array(recoveryNative),
      { canOwn: true },
    );
    py.runPython(sourceArchiveLoader);
    py.runPython(
      "import os,sys; os.chdir('/repo'); sys.path[:0]=['/repo/native/python','/repo','/repo/tools']\nsource_archive_finder = install_source_archive('/repo/runtime-sources.zip','/repo',('native/python/','','tools/'))",
    );
    return py;
  } finally { self.fetch = originalFetch; }
}

function ready(state: State, paths: string[]) {
  return paths.every((path) => {
    let value: unknown = state;
    for (const key of path.split("."))
      value =
        typeof value === "object" && value !== null
          ? (value as Record<string, unknown>)[key]
          : undefined;
    return Array.isArray(value) ? value.length > 0 : Boolean(value);
  });
}
async function boot(
  dat: ArrayBuffer,
  bundle: string,
  config: EngineConfig,
  identities: { path: string; sha256: string }[],
  scene?: SceneInput,
) {
  seedIdentity = identities;
  bootStarted = performance.now();
  bootStages.length = 0;
  send("progress", { message: "Loading camera runtime…", percent: 3 });
  const runtime = await prepareRuntime(bundle),
    protocol = config.boot;
  bootStage("runtime-loaded");
  send("progress", { message: "Checking firmware…", percent: 8 });
  const inputs = await fetch(new URL("firmware-inputs.zip", bundle), {
    cache: "no-store",
  });
  if (!inputs.ok) throw Error("Camera storage inputs are unavailable.");
  const inputBytes = await inputs.arrayBuffer();
  await recovery.identify(
    dat,
    recoverySources,
    recoveryNative,
    inputBytes,
    { protocol, execution: config.execution, firmwareId: config.firmwareId, positions: config.initialPositions, sourceLoader: await digest(new TextEncoder().encode(sourceArchiveLoader)) },
    identities,
  );
  runtime.unpackArchive(inputBytes, "zip", { extractDir: "/repo" });
  runtime.FS.mkdirTree(
    protocol.datPath.slice(0, protocol.datPath.lastIndexOf("/")),
  );
  runtime.FS.writeFile(protocol.datPath, new Uint8Array(dat), { canOwn: true });
  runtime.globals.set("boot_config_json", JSON.stringify(protocol));
  runtime.globals.set("selected_firmware_id", config.firmwareId);
  runtime.runPython(`
import gc, hashlib, importlib, json, functools
from pathlib import Path
for name, expected in json.loads(Path('/repo/web-inputs.json').read_text()).items():
    if hashlib.sha256((Path('/repo') / name).read_bytes()).hexdigest() != expected:
        raise ValueError('Input SHA-256 differs: ' + name)
boot_config = json.loads(boot_config_json)
firmware_id = selected_firmware_id
prepare = boot_config['prepare']
boot_module = importlib.import_module(boot_config['module'])
transport = importlib.import_module(boot_config['sessionModule'])
from native.web.errors import guarded_request
request = functools.partial(guarded_request, transport.request)
use_analytic_scene = transport.use_analytic_scene
set_sensor_scene = transport.set_sensor_scene
session = None
gc.collect()
`);
  bootStage("inputs-verified");
  activeConfig = config;
  return startSession(config, undefined, false, false, [], scene);
}
async function startSession(
  config: EngineConfig,
  files: { path: string; bytes: ArrayBuffer }[] | undefined,
  restarting: boolean,
  replaceCard = false,
  bootInputs: { id: string; stage?: string; position?: string }[] = [],
  scene?: SceneInput,
) {
  const runtime = py!,
    protocol = config.boot;
  const now = new Date();
  const clock = Math.floor(
    (Date.UTC(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
      now.getHours(),
      now.getMinutes(),
      now.getSeconds(),
    ) -
      Date.UTC(2000, 0, 1)) /
      1000,
  );
  runtime.globals.set("clock_seconds_2000", config.execution?.clockSeconds2000 ?? clock);
  if (config.execution?.mode === "replay") recovery.bypassPersistence();
  runtime.globals.set("boot_step", protocol.step);
  if (restarting) recovery.bypassPersistence();
  const resumed = await recovery.restore(
    runtime,
    restarting || config.restart === true,
  );
  if (!resumed) {
    files ??= await initialFiles();
    runtime.runPython(
      "getattr(importlib.import_module(prepare['module']), prepare['function'])(**prepare['options'])",
    );
    runtime.FS.mkdirTree("/card-seed");
    const seeds = files.map((file, index) => {
      const staged = `/card-seed/${index}.bin`;
      runtime.FS.writeFile(staged, new Uint8Array(file.bytes), {
        canOwn: true,
      });
      return { path: file.path, staged };
    });
    runtime.globals.set("card_seeds_json", JSON.stringify(seeds));
    runtime.globals.set("restart_card", restarting);
    runtime.globals.set("replace_card", replaceCard);
    runtime.globals.set("external_inputs_json", JSON.stringify(bootInputs));
    runtime.runPython(`
  card_seeds = json.loads(card_seeds_json)
  card_files = [(item['path'], Path(item['staged']).read_bytes()) for item in card_seeds]
  card_revision = None
  retained_media = None
  if restart_card and not replace_card and not card_files:
      retained_media = transport.retained_card(session)
  elif restart_card and not replace_card:
      card_files, card_revision = transport.files_for_card_restart(session, card_files)
  elif restart_card:
      card_revision = json.loads(transport.card_state(session))['revision'] + 1
  session = None
  browser_history = None
  boot_module.session = None
  boot_module.board = None
  gc.collect()
  boot_module.start(clock_seconds_2000, firmware_id, card_files=card_files, card_revision=card_revision, external_inputs=json.loads(external_inputs_json), card_media=retained_media)
  for item in card_seeds:
      Path(item['staged']).unlink()
  del card_files, card_seeds, retained_media
  `);
    while (true) {
      const progress: { instructions: number; failure?: unknown } = JSON.parse(
        runtime.runPython("boot_module.advance(boot_step)"),
      );
      send("progress", {
        message: `Booting camera · ${progress.instructions.toLocaleString()} / ${protocol.limit.toLocaleString()} instructions`,
        percent: 10 + Math.floor((80 * progress.instructions) / protocol.limit),
      });
      if (progress.failure) throw new EngineError(faultFrom(progress.failure));
      if (progress.instructions >= protocol.limit) break;
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    runtime.runPython("session = boot_module.finish()");
    if (protocol.sceneInput) runtime.runPython("use_analytic_scene(session)");
    bootStage("cold-boot-complete");
  } else {
    runtime.runPython("boot_module.board = session.board");
    send("progress", { message: "Resuming saved camera…", percent: 95 });
  }
  runtime.runPython("transport.configure(session)");
  const replacingScene = protocol.sceneInput && scene;
  const initialImageFrames = replacingScene ? Number(currentState().image_frames ?? 0) : 0;
  if (replacingScene) {
    recordInput("sensor", { width: scene.width, height: scene.height });
    installScene(scene);
  }
  for (const [id, position] of Object.entries(config.initialPositions ?? {})) {
    recordInput("set_position", { id, position });
    runtime.globals.set("initial_position", JSON.stringify({ id, position }));
    runtime.runPython("request(session, 'set_position', initial_position)");
  }
  let state: State = {};
  const displayReady = () => ready(state, protocol.readyWhen)
    && (!replacingScene || Number(state.image_frames ?? 0) > initialImageFrames);
  for (let spent = 0; spent < protocol.readyLimit; spent += protocol.step) {
    if (!resumed || replacingScene) runtime.runPython("session._advance(boot_step)");
    state = currentState();
    const fault = stateFault(state);
    if (fault) throw new EngineError(fault);
    if (displayReady()) break;
    send("progress", { message: "Preparing camera display…", percent: 92 });
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  if (!displayReady())
    throw Error(
      "Camera display did not become ready within its instruction budget.",
    );
  configurePreview();
  recovery.start(runtime);
  bootStage("shooting-frame-ready");
  if (!resumed) await recovery.capture(runtime);
  const frame = currentFrame();
  send("cache", recovery.status(runtime));
  stopped = false;
  errorTrigger = undefined;
  const card = cardState();
  bootStage("ready-reply");
  return { state, frame, card, runtime: {
    ...runtimeMetrics(),
    startupResources: performance.getEntriesByType("resource").map(entry => {
      const resource = entry as PerformanceResourceTiming;
      return {name: resource.name, seconds: resource.duration/1000, transferredBytes: resource.transferSize};
    }),
  } };
}
worker.onmessage = async ({ data }: MessageEvent<Message>) => {
  try {
    if (data.type === "card_files") {
      if (!provideFiles) throw Error("No initial card request is pending.");
      const resolve=provideFiles;provideFiles=undefined;resolve(data.files);return;
    }
    if (data.type === "prepare") {
      await prepareRuntime(data.bundle);
      return;
    }
    if (data.type === "error_report") {
      let result: {
        checkpoint?: ArrayBuffer;
        snapshot: Record<string, unknown>;
      };
      try {
        result = recovery.errorSnapshot(runtime());
      } catch (error) {
        result = { snapshot: { unavailable: String(error) } };
      }
      send(
        "error_report_result",
        {
          id: data.id,
          ...result,
          identity: recovery.identity,
          inputs: recentInputs,
          trigger: errorTrigger,
          runtime: runtimeMetrics(),
        },
        result.checkpoint ? [result.checkpoint] : [],
      );
      return;
    }
    if (data.type === "boot") {
      const result = await boot(
        data.dat,
        data.bundle,
        data.config,
        data.seedIdentity,
        data.scene,
      );
      send("ready", result, [result.frame]);
      return;
    }
    if (data.type === "restart") {
      if (!activeConfig) throw Error("Emulator has not booted.");
      recordInput(
        data.bootInputs?.length
          ? "power_cycle"
          : data.replaceCard
            ? "format_card"
            : "restart_card",
        {
          inputs: data.bootInputs ?? [],
          files: data.cardFiles.map((file) => ({
            path: file.path,
            bytes: file.bytes.byteLength,
          })),
        },
      );
      activeConfig = {
        ...activeConfig,
        initialPositions: {
          ...activeConfig.initialPositions,
          ...data.initialPositions,
        },
      };
      const result = await startSession(
        activeConfig,
        data.cardFiles,
        true,
        data.replaceCard,
        data.bootInputs,
        data.scene,
      );
      stopped = false;
      send("restarted", { id: data.id, ...result }, [result.frame]);
      return;
    }
    const py = runtime();
    if (data.type === "recovery_status") {
      send("cache", recovery.status(py));
      return;
    }
    if (data.type === "rollback") {
      recovery.rollback(py, data.seconds);
      stopped = false;
      errorTrigger = undefined;
      const state = currentState();
      state.input_positions = JSON.parse(
        py.runPython("transport.input_positions(session)"),
      );
      publish(state, data.id, true);
      return;
    }
    if (stopped && data.type !== "card")
      throw new EngineError({
        kind: "exception",
        message: "The camera is paused. Use Undo or Restart to continue.",
        details: {},
      });
    if (data.type === "card") {
      py.globals.set(
        "card_request_json",
        JSON.stringify({
          revision: data.revision ?? null,
          path: data.path,
          present: data.present,
          max_bytes: data.maxBytes ?? 64 * 1024 * 1024,
        }),
      );
      py.runPython(
        'card_request = json.loads(card_request_json)\ncard_revision = card_request["revision"]',
      );
      if (data.action === "state")
        send("card_result", { id: data.id, card: cardState() });
      else if (data.action === "present") {
        recordInput("card_present", { present: data.present });
        py.runPython(
          'transport.set_card_present(session, card_request["present"])',
        );
        send("card_result", { id: data.id, card: cardState() });
      } else if (data.action === "tree") {
        const tree = JSON.parse(
          py.runPython("transport.card_tree(session, card_revision)"),
        );
        send("card_result", { id: data.id, card: cardState(), tree });
      } else if (data.action === "read") {
        const view = py.runPython<PyProxy>(
          'memoryview(transport.read_card_file(session, card_request["path"], card_revision, card_request["max_bytes"]))',
        );
        const buffer = view.getBuffer("u8");
        try {
          const bytes = Uint8Array.from(buffer.data).buffer;
          send("card_result", { id: data.id, card: cardState(), bytes }, [
            bytes,
          ]);
        } finally {
          buffer.release();
          view.destroy();
        }
      }
      return;
    }
    if (data.type === "profile") {
      py.globals.set(
        "browser_profile_json",
        JSON.stringify({ limit: data.limit }),
      );
      const report: State = JSON.parse(
        py.runPython(
          'from native.web.performance import profile\nprofile(session, json.loads(browser_profile_json)["limit"])',
        ),
      );
      const fault = stateFault(report);
      if (fault) throw new EngineError(fault);
      send("profile_result", {
        id: data.id,
        report,
        runtime: runtimeMetrics(),
      });
      return;
    }
    if (data.type === "request") {
      if (!["advance", "state"].includes(data.action))
        recordInput(data.action, data.options);
      py.globals.set("viewer_action", data.action);
      py.globals.set("viewer_options", JSON.stringify(data.options || {}));
      const state: State = requestTimings.measure(data.action, () =>
        JSON.parse(
          py.runPython("request(session, viewer_action, viewer_options)"),
        ),
      );
      py.runPython(
        "boot_module.board = session.board\nboot_module.trim_diagnostics()",
      );
      const fault = stateFault(state);
      if (fault) throw new EngineError(fault);
      if (await recovery.capture(py)) send("cache", recovery.status(py));
      publish(
        state,
        data.id,
        (data.action === "state" && data.options?.pixels !== false) ||
          data.action === "key",
      );
      return;
    }
    if (data.type === "sensor_reset") {
      recordInput("sensor_reset");
      py.runPython("use_analytic_scene(session)");
      configurePreview();
      send("sensor_ready", { id: data.id });
      return;
    }
    if (data.type === "sensor") {
      recordInput("sensor", {
        width: data.width,
        height: data.height,
        sequence: data.sequence,
      });
      installScene(data);
      configurePreview();
      send("sensor_ready", { id: data.id, sequence: data.sequence, runtime: runtimeMetrics() });
    }
  } catch (error) {
    if (data.type === "card") {
      send("error", { id: data.id, error: String(error) });
      return;
    }
    stopped = true;
    errorTrigger ??=
      data.type === "boot"
        ? { type: data.type, config: data.config }
        : data.type === "sensor"
          ? { type: data.type, width: data.width, height: data.height }
          : { ...data };
    send("error", {
      id: "id" in data ? data.id : null,
      fault: {
        ...faultFrom(error),
        undoSeconds: recovery.canRollback(py, FAULT_UNDO_SECONDS)
          ? FAULT_UNDO_SECONDS : undefined,
      },
      identity: recovery.identity,
    });
  }
};
