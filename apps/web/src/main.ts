import "./style.css";
import { validateCatalog, type Catalog, type Firmware } from "./catalog";
import { validateIndex, verifyPatch } from "./patch/catalog";
import { sha256 } from "./patch/patcher";
import { createEngine } from "./engine";
import { EngineError, FAULT_UNDO_SECONDS, faultFrom, type EngineFault } from "./engine/fault";
import { cachedFetch } from "./asset-cache";
import type { Engine, CardFile, BootMedia } from "./engine/api";
import { errorReportZip, type ReportSubmission } from "./engine/report";
import { ModelViewer, type ModelData } from "./viewer/model";
import { InputPanel } from "./ui/controls";
import { ConnectionsPanel } from "./ui/connections";
import { CardBrowser, downloadBlob } from "./ui/card";
import { readPendingReport, storePendingReport, deliverReport, type PendingReport } from "./ui/report-delivery";
import { simulatorIssue, type SimulatorIssueContext } from "./ui/simulator-feedback";
import { FaultPanel } from "./ui/fault";
import { cameraCard, cameraTable, type CameraSupport } from "./ui/cameras";
import { personalNote, noteCameraSupport } from "./ui/personal-note";
import type { ReplayScenario } from "./scenarios";
import type { DevicePanel, PowerCombo } from "./ui/layout";
import generations from "../devices/generations.json";

performance.mark("camera-module-ready");

declare const __FUJIMOD_LOCAL_DAT__: boolean;
declare const __FUJIMOD_SIMULATOR_BUILD__: string;
declare const __FUJIMOD_CATALOG__: string;
const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const adapters = import.meta.glob<{
  model?: ModelData;
  panel: DevicePanel;
  initialPositions: Record<string, string>;
  preview?: string;
}>("../devices/*/index.ts");
const note = document.querySelector<HTMLElement>(".personal-note")!;
personalNote(note);
const features = $<HTMLDetailsElement>("features");
const openFeatures = () => {
  if (location.hash === "#features") features.open = true;
};
window.addEventListener("hashchange", openFeatures);
document.querySelector('a[href="#features"]')!.addEventListener("click", () => {
  features.open = true;
});
openFeatures();
const lines = document.createElementNS("http://www.w3.org/2000/svg", "svg");
lines.id = "leaders";
$("workspace").append(lines);
const viewer = new ModelViewer($("viewer"), lines, $("workspace"));
const faultPanel = new FaultPanel($("engine-status"));
const panel = new InputPanel(
  $("controls"),
  viewer,
  (error) => showFault(faultFrom(error)),
  runCombo,
);
const connections = new ConnectionsPanel($("connections"));
const card = new CardBrowser(
  $("card-section"),
  $("card-files"),
  $("card-status"),
  async (value) => {
    await engine?.insertCard(value);
  },
  async () => {
    await restartOperation(() => engine!.formatCard(progress));
  },
);
let catalog: Catalog,
  release: ReturnType<typeof validateIndex> | undefined,
  engine: Engine | undefined;
let generation = 0,
  selectedFile: (() => Promise<File>) | undefined,
  selectedFirmware: Firmware | undefined,
  latestFault: EngineFault | undefined;
let firmwareHash: string | undefined;
let manualSelection = false;
let build = "Development",
  modelLoaded: string | undefined,
  modelData: ModelData | undefined;
let patchWorker: Worker | undefined;
let preparedEngine: { value: Engine; bundle: string; adapter: string } | undefined;
let modelLoading: Promise<void> = Promise.resolve();
let replayScenario: ReplayScenario | undefined;
function status(message: string, error = false) {
  $("status").textContent = message;
  $("status").classList.toggle("error", error);
}
function progress(message: string, percent: number) {
  status(message);
  const bar = $<HTMLProgressElement>("boot-progress");
  bar.hidden = false;
  bar.value = percent;
}
function title(speed?: number) {
  $("camera-title").textContent = selectedFirmware
    ? `${selectedFirmware.model} · ${selectedFirmware.version}${speed === undefined ? "" : ` · ${speed.toFixed(2)}×`}`
    : "";
}
function showFault(fault: EngineFault) {
  latestFault = fault;
  panel.disable();
  $("live-label").textContent = "STOPPED";
  $("boot-progress").hidden = true;
  status(fault.message, true);
  const canUndo = !!engine && fault.undoSeconds === FAULT_UNDO_SECONDS;
  $<HTMLButtonElement>("undo").hidden = !canUndo;
  $<HTMLButtonElement>("undo").disabled = !canUndo;
  $<HTMLButtonElement>("restart").disabled = !selectedFile;
  faultPanel.show(fault, {
    firmware: selectedFirmware?.id,
    build,
    tick: window.cameraPerformance
      ? window.cameraPerformance.guestTicks
      : undefined,
  });
  viewer.fault(true);
}
async function undo() {
  const button = $<HTMLButtonElement>("undo");
  if (!engine || button.disabled || latestFault?.undoSeconds !== FAULT_UNDO_SECONDS) return;
  button.disabled = true;
  viewer.fault(true);
  try {
    const positions = await engine.rollback(FAULT_UNDO_SECONDS);
    viewer.fault(false);
    faultPanel.hide();
    panel.resume();
    panel.synchronize(positions);
    $("live-label").textContent = "LIVE";
    status("Camera resumed");
  } catch (error) {
    showFault(faultFrom(error));
  }
}
$("undo").onclick = () => void undo();
$("restart").onclick = async () => {
  if (selectedFile) {
    const reload = selectedFile;
    try { await processFile(await reload(), true, undefined, reload); }
    catch (error) { showFault(faultFrom(error)); }
  }
};
const feedbackDialog = $<HTMLDialogElement>("feedback-dialog");
const feedbackDescription = $<HTMLTextAreaElement>("feedback-description");
let feedbackContext: SimulatorIssueContext = { event: "feedback", modBuild: "Not identified", simulatorBuild: __FUJIMOD_SIMULATOR_BUILD__ };
let preparingReport = false;
function openFeedback(event: "feedback" | "crash") {
  if (preparingReport || feedbackDialog.open) return;
  feedbackContext = {
    event,
    simulatorBuild: __FUJIMOD_SIMULATOR_BUILD__,
    modBuild: !selectedFirmware ? "Not identified"
      : release?.patches[selectedFirmware.sha256]
        ? `${release.release}${release.built_utc ? ` / ${release.built_utc}` : " / Build date unavailable"}`
        : "Stock firmware (no mods)",
    camera: selectedFirmware && { model: selectedFirmware.model, id: selectedFirmware.id },
    fault: event === "crash" && latestFault ? structuredClone(latestFault) : undefined,
  };
  feedbackDescription.value = "";
  $<HTMLImageElement>("feedback-preview").src = viewer.captureView();
  feedbackDialog.returnValue = "cancel";
  let describe!: (value: ReportSubmission) => void;
  const submission = new Promise<ReportSubmission>(resolve => { describe = resolve; });
  // Capture at the moment feedback is requested, before the user writes a comment.
  const capture = engine ? engine.errorReport(submission) : submission.then(value => errorReportZip({
    ...value,
    error: latestFault,
    browser: { userAgent: navigator.userAgent },
    identity: { firmwareSha256: firmwareHash },
    snapshot: { unavailable: "Runtime did not initialize." },
  }));
  const result = capture.then(blob => ({ blob }), error => ({ error }));
  const prepare = $<HTMLButtonElement>("feedback-prepare");
  const issue = $<HTMLAnchorElement>("feedback-issue");
  const message = $("feedback-status");
  prepare.hidden = false;
  prepare.disabled = false;
  prepare.textContent = "Download and report";
  issue.hidden = message.hidden = true;
  feedbackDescription.readOnly = false;
  let submitted: ReturnType<typeof simulatorIssue> | undefined;
  let pending: PendingReport | undefined;
  let closed = false;
  feedbackDialog.querySelector("form")!.onsubmit = async event => {
    if ((event.submitter as HTMLButtonElement | null)?.value !== "download") return;
    event.preventDefault();
    if (preparingReport) return;
    preparingReport = true;
    prepare.disabled = true;
    feedbackDescription.readOnly = true;
    submitted ??= simulatorIssue(feedbackDescription.value, feedbackContext);
    describe(submitted);
    message.textContent = "Preparing report…";
    message.hidden = false;
    try {
      if (!pending) {
        const report = await result;
        if (closed) return;
        if ("error" in report) throw report.error;
        pending = {blob: report.blob, name: `fujimod-feedback-${Date.now()}.zip`,
          issueURL: submitted.issue.url};
      }
      await storePendingReport(pending);
      if (closed) return;
      savedReport = pending;
      $("feedback-recover").hidden = false;
      issue.href = pending.issueURL;
      issue.hidden = message.hidden = true;
      deliverReport(pending, downloadBlob);
      issue.hidden = false;
      prepare.textContent = "Download again";
    } catch (error) {
      if (!closed) message.textContent = `Could not prepare report: ${String(error)}`;
    } finally {
      preparingReport = false;
      prepare.disabled = false;
    }
  };
  feedbackDialog.onclose = () => {
    closed = true;
    describe(submitted ?? simulatorIssue(feedbackDescription.value, feedbackContext));
  };
  feedbackDialog.showModal();
  feedbackDescription.focus();
}
$("error-report").onclick = () => openFeedback("crash");
$("feedback-open").onclick = () => openFeedback("feedback");
let savedReport: PendingReport | undefined;
void readPendingReport().then(report => {
  if (report) { savedReport = report; $("feedback-recover").hidden = false; }
});
$("feedback-recover").onclick = () => {
  if (savedReport) downloadBlob(savedReport.blob, savedReport.name);
};

async function runCombo(combo: PowerCombo) {
  await restartOperation(() => engine!.powerCycle(combo.inputs, progress));
}
async function restartOperation(operation: () => Promise<void>) {
  if (!engine) return;
  panel.disable();
  $("live-label").textContent = "RESTARTING";
  try {
    await operation();
    viewer.fault(false);
    faultPanel.hide();
    latestFault = undefined;
    panel.resume();
    card.connect(engine);
    $("live-label").textContent = "LIVE";
  } catch (error) {
    showFault(faultFrom(error));
  } finally {
    $("boot-progress").hidden = true;
  }
}
function credit(data: ModelData) {
  const host = $("model-credit");
  host.replaceChildren();
  const a = (text: string, url: string) => {
    const link = document.createElement("a");
    link.textContent = text;
    link.href = url;
    return link;
  };
  host.append(
    '"',
    a(data.credit.title, data.credit.url),
    '" by ',
    a(data.credit.author, data.credit.authorUrl),
    " · ",
    a("CC BY 4.0", data.credit.license),
  );
}
async function loadDevice(body: string) {
  const load = adapters[`../devices/${body}/index.ts`];
  if (!load) return;
  const device = await load();
  if (device.model && modelLoaded !== body) {
    const model = device.model;
    modelLoaded = body;
    modelData = model;
    modelLoading = viewer.load(model).then(() => credit(model));
    void modelLoading.catch(error => showFault(faultFrom(error)));
  }
  return device;
}
function reset() {
  panel.clear();
  card.connect();
  connections.update([]);
  viewer.setSceneConsumer();
  engine?.dispose();
  engine = undefined;
  patchWorker?.terminate();
  patchWorker = undefined;
  viewer.fault(false);
  faultPanel.hide();
  latestFault = undefined;
  window.cameraPerformance = undefined;
  window.profileCamera = undefined;
  $("live-label").textContent = "";
  $("workspace").classList.add("idle");
  $("panel").hidden = true;
  $("viewer-bar").hidden = true;
  $("firmware-prompt").hidden = false;
  $("idle-cameras").hidden = false;
  $("boot-progress").hidden = true;
  title();
}
function makeEngine(config: NonNullable<Firmware['emulate']> & {firmwareId: string}) {
  let value: Engine;
  value = createEngine(config, error => {
    if (engine === value) showFault(faultFrom(error));
    else if (preparedEngine?.value === value) { preparedEngine = undefined; value.dispose(); }
  });
  return value;
}
function bootMedia(file: File): BootMedia {
  const index = release, selected = selectedFirmware!;
  const firmware = index?.firmwares.find(entry => entry.sha256 === selected.sha256);
  if (!index || !firmware || !index.patches[selected.sha256]) return {identities:Promise.resolve([]),load:async()=>[]};
  const metadata = (async () => {
    const response = await fetch(new URL(`artifacts/${index.patches[selected.sha256]}`, document.baseURI));
    if (!response.ok) throw Error("The release patch could not be downloaded.");
    return verifyPatch(index, firmware, new Uint8Array(await response.arrayBuffer()));
  })();
  return {
    identities:metadata.then(meta => ["ffboot","ffsys"].map(name => ({path:`/${name}.bin`,sha256:meta.outputs[name as "ffboot"|"ffsys"].sha256}))),
    load:()=>patchedFiles(file),
  };
}
function patchedFiles(file: File): Promise<CardFile[]> {
  if (!release?.patches[selectedFirmware!.sha256]) return Promise.resolve([]);
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./patch/worker.ts", import.meta.url), {
      type: "module",
    });
    patchWorker = worker;
    worker.onmessage = ({ data }) => {
      if (data.stage) return;
      worker.terminate();
      if (patchWorker === worker) patchWorker = undefined;
      if (data.error) return reject(Error(data.error));
      if (data.unsupported) return resolve([]);
      resolve(
        ["ffboot", "ffsys"].map((name) => ({
          path: `/${name}.bin`,
          bytes: new Uint8Array(data.result[name]),
        })),
      );
    };
    worker.onerror = (error) => {
      worker.terminate();
      reject(Error(error.message));
    };
    worker.postMessage({
      file,
      index: release,
      artifactBase: new URL("artifacts/", document.baseURI).href,
    });
  });
}
async function processFile(file?: File, restart = false, preparedBytes?: ArrayBuffer,
  reload?: () => Promise<File>) {
  if (!file) return;
  performance.mark("camera-file-received");
  const current = ++generation;
  reset();
  selectedFile = undefined;
  selectedFirmware = undefined;
  firmwareHash = undefined;
  try {
    if (!file.size || file.size > 256 * 1024 * 1024)
      throw Error("Choose a firmware DAT smaller than 256 MB.");
    const bytes = preparedBytes ?? await file.arrayBuffer(),
      hash = await sha256(new Uint8Array(bytes));
    if (current !== generation) return;
    performance.mark("camera-dat-verified");
    firmwareHash = hash;
    const entry: Firmware | undefined =
      catalog.firmwares.find((item) => item.sha256 === hash) ??
      (release?.firmwares.find((item) => item.sha256 === hash) as
        | Firmware
        | undefined);
    if (!entry) throw Error(`Unsupported firmware · SHA-256 ${hash}`);
    if (file.size !== entry.bytes)
      throw Error("Firmware size differs from the catalog.");
    selectedFile = reload ?? (() => Promise.resolve(file));
    if (replayScenario && entry.id !== replayScenario.firmwareId)
      throw Error("The replay scenario requires firmware " + replayScenario.firmwareId);
    selectedFirmware = entry;
    const device = entry.body
      ? await loadDevice(entry.body)
      : undefined;
    if (current !== generation) return;
    if (!entry.emulate || !device)
      throw Error("Emulation unsupported for this firmware.");
    $("model-credit").hidden = !device.model;
    $("view-3d").hidden = !device.model;
    setView(device.model ? "3d" : "lcd");
    $("workspace").classList.remove("idle");
    $("panel").hidden = false;
    $("viewer-bar").hidden = false;
    $("firmware-prompt").hidden = true;
    $("idle-cameras").hidden = true;
    title();
    const active = preparedEngine?.bundle === entry.emulate.bundle && preparedEngine.adapter === entry.emulate.adapter
      ? preparedEngine.value : makeEngine({...entry.emulate, firmwareId:entry.id});
    if (preparedEngine && preparedEngine.value !== active) preparedEngine.value.dispose();
    preparedEngine = undefined;
    engine = active;
    if (entry.emulate.boot.sceneInput) viewer.setSceneConsumer(scene => active.setScene(scene));
    active.metrics((metrics) => {
      if (current !== generation) return;
      window.cameraPerformance = metrics;
      title(metrics.speed);
      $("boot-progress").hidden = true;
    });
    window.profileCamera = (limit) => active.profile(limit);
    active.connections(rows => {
      if (current === generation) connections.update(rows);
    });
    active.frames((frame) => {
      if (current === generation) viewer.frame(frame);
    });
    const seeds = bootMedia(file);
    if (current !== generation) return;
    const started = performance.now();
    performance.mark("camera-engine-boot");
    const inputs = await active.boot(
      bytes,
      { ...entry.emulate, firmwareId:entry.id, initialPositions: device.initialPositions, restart,
        execution: replayScenario ? { mode: "replay", clockSeconds2000: replayScenario.clockSeconds2000 } : undefined },
      progress,
      seeds,
    );
    if (current !== generation) return;
    performance.mark("camera-engine-ready");
    await modelLoading;
    if (current !== generation) return;
    performance.mark("camera-model-ready");
    panel.render(inputs, active, device.panel, device.initialPositions);
    card.connect(active);
    $("boot-progress").hidden = true;
    $("live-label").textContent = "LIVE";
    $("viewer").dataset.bootSeconds = (
      (performance.now() - started) /
      1000
    ).toFixed(3);
    status("Camera ready");
    if (replayScenario) {
      setView("lcd");
      $("workspace").scrollIntoView({ block: "start", behavior: "instant" });
      panel.disable();
      await active.replay(replayScenario.steps);
      $("live-label").textContent = "REPLAY COMPLETE";
      status("The scenario completed without an execution fault.");
    }
  } catch (error) {
    if (current === generation && !(error instanceof EngineError))
      showFault(faultFrom(error));
  }
}
$<HTMLInputElement>("firmware-file").onchange = (event) => {
  manualSelection = true;
  const input = event.target as HTMLInputElement;
  void processFile(input.files?.[0]);
  input.value = "";
};
async function tryFirmware(firmware: Firmware) {
  manualSelection = true;
  if (__FUJIMOD_LOCAL_DAT__) {
    const reload = async () => {
      const response = await cachedFetch(`/local/firmware/${firmware.id}.dat`);
      if (!response.ok) return undefined;
      return new File([await response.arrayBuffer()], `${firmware.id}.DAT`);
    };
    const file = await reload();
    if (file) {
      await processFile(file, false, undefined, async () => {
        const file = await reload();
        if (!file) throw Error('The local firmware could not be read.');
        return file;
      });
      return;
    }
  }
  status(`Choose the ${firmware.model} firmware DAT`);
  $<HTMLInputElement>("firmware-file").click();
}
$("viewer").ondragover = (event) => {
  event.preventDefault();
  $("viewer").classList.add("dragging");
};
$("viewer").ondragleave = () => $("viewer").classList.remove("dragging");
$("viewer").ondrop = (event) => {
  manualSelection = true;
  event.preventDefault();
  $("viewer").classList.remove("dragging");
  if (event.dataTransfer?.files[0])
    void processFile(event.dataTransfer.files[0]);
};
$("face-screen").onclick = () => viewer.faceScreen();
$("face-lens").onclick = () => viewer.faceLens();
function setView(mode: "3d" | "lcd") {
  viewer.view(mode);
  for (const value of ["3d", "lcd"]) {
    $(`view-${value}`).classList.toggle("on", value === mode);
    $(`view-${value}`).setAttribute("aria-pressed", String(value === mode));
  }
}
for (const mode of ["3d", "lcd"] as const)
  $(`view-${mode}`).onclick = () => setView(mode);
window.addEventListener("beforeunload", () => engine?.dispose());
try {
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("scenario")) {
    const { scenarioFromUrl } = await import("./scenarios");
    replayScenario = scenarioFromUrl(location.search);
    if (!__FUJIMOD_LOCAL_DAT__) throw Error("Replay scenarios require a configured local firmware DAT.");
  }
  const response = await fetch(new URL(__FUJIMOD_CATALOG__, document.baseURI));
  if (!response.ok) throw Error("Firmware catalog unavailable.");
  catalog = validateCatalog(await response.json());
  const patchResponse = await fetch(
    new URL("artifacts/patches.json", document.baseURI),
  );
  if (
    patchResponse.ok &&
    patchResponse.headers.get("content-type")?.includes("json")
  )
    release = validateIndex(await patchResponse.json());
  if (release && !import.meta.env.DEV) {
    build = release.release;
    $("release-label").textContent = `Build ${build}`;
    $<HTMLAnchorElement>("release-label").href =
      `https://github.com/justjake/fujimod/releases/tag/${encodeURIComponent(build)}`;
  }
  const entries = [...catalog.firmwares];
  for (const entry of release?.firmwares ?? [])
    if (!entries.some((known) => known.sha256 === entry.sha256))
      entries.push(entry);
  const cameras: CameraSupport[] = entries
    .filter((entry) => entry.emulate || release?.patches[entry.sha256])
    .map((firmware) => ({
      firmware,
      mod: Boolean(release?.patches[firmware.sha256]),
      emu: Boolean(firmware.emulate),
    }));
  const selectFirmware = (firmware: Firmware) => {
    void tryFirmware(firmware).catch(error => showFault(faultFrom(error)));
  };
  $("camera-cards").replaceChildren(...cameras.map(camera => cameraCard(camera, selectFirmware)));
  cameraTable($("camera-table"), generations, cameras, selectFirmware);
  noteCameraSupport(note, cameras);
  performance.mark("camera-catalog-ready");
  const localDat = __FUJIMOD_LOCAL_DAT__ ? cachedFetch("/local/firmware.dat") : undefined;
  const first = cameras.find((camera) => camera.firmware.model_asset);
  if (localDat && first?.firmware.emulate) {
    const config = first.firmware.emulate;
    const value = makeEngine({...config,firmwareId:first.firmware.id});
    preparedEngine = {value, bundle:config.bundle, adapter:config.adapter};
    value.prepare({...config,firmwareId:first.firmware.id});
  }
  if (first?.firmware.model_asset) {
    const device = await loadDevice(first.firmware.model_asset);
    if (device)
      $("viewer").style.setProperty("--body-preview", `url(${device.preview})`);
  }
  if (localDat) {
    const dat = await localDat;
    if (dat.ok && !manualSelection) {
      const bytes = await dat.arrayBuffer();
      // The local preview can reread its verified DAT on Restart. Retaining a
      // synthetic File here would keep another entire DAT in browser memory.
      const reload = async () => {
        const response = await cachedFetch("/local/firmware.dat");
        if (!response.ok) throw Error("The local firmware could not be read.");
        return new File([await response.arrayBuffer()], "FPUPDATE.DAT");
      };
      if (!manualSelection)
        await processFile(new File([bytes], "FPUPDATE.DAT"), false, bytes, reload);
    }
  }
} catch (error) {
  showFault(faultFrom(error));
}
