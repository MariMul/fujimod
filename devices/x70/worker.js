let py = null;
let session = null;
let bundle = null;
let peakWasmMemoryBytes = 0;

function runtimeMetrics() {
  const wasmMemoryBytes = py?._module.HEAPU8.buffer.byteLength || 0;
  peakWasmMemoryBytes = Math.max(peakWasmMemoryBytes, wasmMemoryBytes);
  return { wasmMemoryBytes, peakWasmMemoryBytes };
}

function send(type, fields = {}, transfer = []) {
  self.postMessage({ type, ...fields }, transfer);
}

function currentFrame() {
  const view = py.runPython("memoryview(session.raw)");
  const buffer = view.getBuffer("u8");
  try {
    return buffer.data.slice().buffer;
  } finally {
    buffer.release();
    view.destroy();
  }
}

function currentState() {
  return JSON.parse(py.runPython("json.dumps(session.sample())"));
}

function publish(state, id = null, includeFrame = true) {
  const frame = includeFrame ? currentFrame() : new ArrayBuffer(0);
  send("result", { id, state, frame, runtime: runtimeMetrics() }, frame.byteLength ? [frame] : []);
}

async function prepareRuntime() {
  if (py) return;
  const { loadPyodide } = await import(/* @vite-ignore */ new URL("pyodide/pyodide.mjs", bundle).href);
  py = await loadPyodide({ indexURL: new URL("pyodide/", bundle).href });
  await py.loadPackage(["numpy", "pillow"]);
  py.unpackArchive(await (await fetch(new URL("sources.zip", bundle))).arrayBuffer(), "zip", {
    extractDir: "/repo",
  });
  py.FS.writeFile(
    "/repo/native/python/camera_native/_native.so",
    new Uint8Array(await (await fetch(new URL("_native.so", bundle))).arrayBuffer()),
    { canOwn: true },
  );
  py.runPython(
    "import os,sys; os.chdir('/repo'); sys.path[:0]=['/repo/native/python','/repo','/repo/tools']",
  );
}

async function boot(dat) {
  send("progress", { message: "Loading camera runtime…", percent: 3 });
  await prepareRuntime();
  send("progress", { message: "Checking firmware…", percent: 8 });
  const inputResponse = await fetch(new URL("firmware-inputs.zip", bundle), {
    cache: "no-store",
  });
  if (!inputResponse.ok) throw Error("Camera storage inputs are unavailable.");
  py.unpackArchive(await inputResponse.arrayBuffer(), "zip", {
    extractDir: "/repo",
  });
  py.FS.mkdirTree("/repo/firmware/original");
  py.FS.writeFile("/repo/firmware/original/FPUPDATE.DAT", new Uint8Array(dat), { canOwn: true });
  py.runPython(`
import gc, hashlib, json
from pathlib import Path
for name, expected in json.loads(Path('/repo/web-inputs.json').read_text()).items():
    if hashlib.sha256((Path('/repo') / name).read_bytes()).hexdigest() != expected:
        raise ValueError('Input SHA-256 differs: ' + name)
from tools.unpack_x70 import main as map_dat
map_dat(export_strings=False)
from native.web.boot import start as cold_start, advance as cold_advance, finish as cold_finish
from native.web.session import request, use_analytic_scene
session = None
gc.collect()
`);
  const now = new Date();
  const seconds2000 = Math.floor(
    (Date.UTC(
      now.getFullYear(), now.getMonth(), now.getDate(),
      now.getHours(), now.getMinutes(), now.getSeconds(),
    ) - Date.UTC(2000, 0, 1)) / 1000,
  );
  py.globals.set("clock_seconds_2000", seconds2000);
  py.runPython("cold_start(clock_seconds_2000)");
  const limit = 100_000_000;
  while (true) {
    const progress = JSON.parse(py.runPython("cold_advance(2_000_000)"));
    send("progress", {
      message: `Booting camera · ${progress.instructions.toLocaleString()} / ${limit.toLocaleString()} instructions`,
      percent: 10 + Math.floor(80 * progress.instructions / limit),
    });
    if (progress.failure) throw Error(JSON.stringify(progress.failure));
    if (progress.instructions >= limit) break;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  py.runPython("session = cold_finish()");
  py.runPython("use_analytic_scene(session)");
  const hinge = JSON.parse(py.runPython(
    "request(session, 'set_position', '{\"id\":\"lcd_tilt\",\"position\":\"normal\"}')",
  ));
  let state;
  // Mode 3 precedes the REC completion and first UI composition. Continue
  // stock scheduling until the shooting overlay is actually on the LCD.
  for (let slices = 0; slices < 25; slices++) {
    py.runPython("session._advance(2_000_000)");
    state = currentState();
    if (state.failure || state.execution_error) {
      throw Error(JSON.stringify(state.failure || state.execution_error));
    }
    if (state.rec_initialization_idle && state.display.last_frame?.graphics?.length) break;
    send("progress", { message: "Preparing shooting display…", percent: 92 });
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  if (!state?.rec_initialization_idle || !state.display.last_frame?.graphics?.length) {
    throw Error("Stock shooting display did not become ready within 50,000,000 further instructions.");
  }
  const frame = currentFrame();
  send("ready", { state, frame, uiMode: state.ui.mode, hinge, runtime: runtimeMetrics() },
       frame.byteLength ? [frame] : []);
}

function handleRequest(message) {
  py.globals.set("viewer_action", message.action);
  py.globals.set("viewer_options", JSON.stringify(message.options || {}));
  const state = JSON.parse(
    py.runPython("request(session, viewer_action, viewer_options)"),
  );
  publish(state, message.id, message.action === "state" || message.action === "key");
}

self.onmessage = async ({ data }) => {
  try {
    if (data.type === "boot") {
      bundle = data.bundle;
      await boot(data.dat);
      return;
    }
    if (data.type === "request") {
      handleRequest(data);
      return;
    }
    if (data.type === "sensor_reset") {
      py.runPython("use_analytic_scene(session)");
      send("sensor_ready");
      return;
    }
    if (data.type === "sensor") {
      const bytes = py.toPy(new Uint8Array(data.rgb));
      py.globals.set("sensor_rgb_buffer", bytes);
      py.globals.set("sensor_width", data.width);
      py.globals.set("sensor_height", data.height);
      py.runPython(
        "from native.web.session import set_sensor_scene\nset_sensor_scene(session, sensor_width, sensor_height, bytes(sensor_rgb_buffer))",
      );
      py.globals.delete("sensor_rgb_buffer");
      bytes.destroy();
      send("sensor_ready", { sequence: data.sequence });
    }
  } catch (error) {
    send("error", { id: data.id ?? null, error: String(error) });
  }
};
