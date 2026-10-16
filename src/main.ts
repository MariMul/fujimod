import { CardBrowser } from './ui/card';
import { EngineError, faultFrom, type EngineFault } from './engine/fault';
import './style.css';
import { sha256 } from './patch/patcher.ts';
import { validateIndex } from './patch/catalog.ts';
import { cardZip } from './patch/download.ts';
import { validateCatalog, type Catalog, type Firmware } from './catalog';
import { createEngine } from './engine';
import type { Engine, CardFile } from './engine/api';
import { ModelViewer, type ModelData } from './viewer/model';
import { InputPanel } from './ui/controls';
import { SensorInput } from './ui/sensor';

declare const __FUJIMOD_LOCAL_DAT__: boolean;
declare const __FUJIMOD_CATALOG__: string;
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const adapters = import.meta.glob<{ model: ModelData; initialPositions: Record<string, string> }>('../devices/*/index.ts');
// Vite's glob is relative to this source file; data selects the implementation.
const viewer = new ModelViewer($('viewer'), document.getElementById('leaders') as unknown as SVGSVGElement, $('workspace'));
const panel = new InputPanel($('controls'), viewer, error => { if (!(error instanceof EngineError)) status(String(error), true); });
const sensor = new SensorInput(message => { $('sensor-status').textContent = message; });
let catalog: Catalog;
let release: ReturnType<typeof validateIndex> | undefined;
let engine: Engine | undefined;
let selectedFile: File | undefined;
let selectedFirmware: Firmware | undefined;
let patchWorker: Worker | undefined;
let generation = 0;
let downloadUrls: string[] = [];
let patchSeeds: CardFile[] = [];
let cameraReady = false;
const cardBrowser = new CardBrowser($('card-section'), $('card-files'), $('card-status'));
function status(message: string, error = false) {
  $('status').textContent = message; $('status').classList.toggle('error', error);
  $('status').setAttribute('role', error ? 'alert' : 'status');
}
function showFault(fault: EngineFault) {
  cameraReady = false; $<HTMLButtonElement>('try-sd').disabled = true;
  status(fault.message, true); panel.disable(); $('live-label').textContent = 'STOPPED'; sensor.connect();
  $('boot-progress').hidden = true; $('recovery-actions').hidden = false;
  const details = $<HTMLDetailsElement>('fault-details'); details.hidden = false; details.open = false;
  const fields = $('fault-fields'); fields.replaceChildren();
  const append = (name: string, value: unknown) => {
    if (value && typeof value === 'object') {
      for (const [key, item] of Object.entries(value)) append(`${name} · ${key}`, item);
    } else if (value !== undefined) {
      const term = document.createElement('dt'), text = document.createElement('dd');
      term.textContent = name; text.textContent = String(value); fields.append(term, text);
    }
  };
  append('Kind', fault.kind); for (const [key, value] of Object.entries(fault.details)) append(key, value);
}
$('undo').onclick = async () => {
  const button = $<HTMLButtonElement>('undo'); button.disabled = true;
  try {
    await engine?.rollback(10); cameraReady = true; $<HTMLButtonElement>('try-sd').disabled = false; panel.resume(); sensor.connect(engine);
    $('recovery-actions').hidden = true; $('fault-details').hidden = true;
    $('live-label').textContent = 'LIVE'; status('Camera resumed');
  } catch { status('Undo is unavailable for this point. Restart the camera to continue.', true); }
  finally { button.disabled = false; }
};
$('restart').onclick = () => { void processFile(selectedFile, true); };
function reset() {
  $('recovery-actions').hidden = true; $('fault-details').hidden = true;
  cardBrowser.connect(); cameraReady = false; patchSeeds = []; $('try-sd').hidden = true; $('camera-speed').textContent = ''; window.cameraPerformance = undefined; window.profileCamera = undefined;
  panel.clear(); engine?.dispose(); engine = undefined; sensor.connect();
  patchWorker?.terminate(); patchWorker = undefined;
  for (const url of downloadUrls) URL.revokeObjectURL(url); downloadUrls = [];
  viewer.placeholder();
  for (const id of ['panel','scene-controls','patch-section','result','model-credit','view-actions','flat-lcd']) $(id).hidden = true;
  $('live-label').textContent = ''; $('camera-title').textContent = '';
  $('firmware-prompt').hidden = false; $('change-file').hidden = true;
  $('downloads').replaceChildren(); $('boot-progress').hidden = true; $<HTMLButtonElement>('build-patch').disabled = false;
}
function credit(data: ModelData) {
  const footer = $('model-credit'); footer.replaceChildren();
  const link = (text: string, url: string) => { const a = document.createElement('a'); a.textContent = text; a.href = url; return a; };
  footer.append(link(data.credit.title, data.credit.url), ' by ', link(data.credit.author, data.credit.authorUrl), ' · ', link('CC BY 4.0', data.credit.license)); footer.hidden = false;
}
async function processFile(file?: File, restart = false) {
  if (!file) return;
  const current = ++generation; reset(); selectedFile = undefined; selectedFirmware = undefined;
  if (!catalog) { status('The firmware catalog is unavailable.', true); return; }
  if (!file.size || file.size > 256 * 1024 * 1024) { status('Choose a firmware DAT smaller than 256 MB.', true); return; }
  status('Checking firmware…');
  try {
    const bytes = await file.arrayBuffer(); const digest = await sha256(new Uint8Array(bytes));
    if (current !== generation) return;
    // A release can add patch-only bodies without updating this page source.
    const entry: Firmware | undefined = catalog.firmwares.find(entry => entry.sha256 === digest) ?? release?.firmwares.find((entry: Firmware) => entry.sha256 === digest);
    if (!entry) { status(`Unsupported firmware · SHA-256 ${digest}`, true); return; }
    if (file.size !== entry.bytes) throw Error('Firmware size differs from the catalog.');
    selectedFile = file; selectedFirmware = entry;
    $('firmware-prompt').hidden = true; $('change-file').hidden = false;
    $('camera-title').textContent = `${entry.model} · ${entry.version}`;
    const hasPatch = Boolean(release?.patches[digest]);
    $('patch-section').hidden = !hasPatch;
    const moduleId = entry.model_asset;
    const load = moduleId && adapters[`../devices/${moduleId}/index.ts`];
    const device = load ? await load() : undefined;
    if (current !== generation) return;
    let hasModel = false;
    const modelReady = entry.model_asset && device ? viewer.load(device.model).then(() => {
      if (current !== generation) return;
      hasModel = true; credit(device.model); $('view-actions').hidden = false;
    }).catch(() => { if (current === generation) status('3D model unavailable.'); }) : Promise.resolve();
    if (current !== generation) return;
    if (!entry.emulate) { await modelReady; status(hasPatch ? 'SD patch available.' : hasModel ? 'Emulation unsupported.' : 'Unsupported firmware.'); return; }
    const activeEngine = createEngine(entry.emulate.adapter, (error: Error) => {
      if (current !== generation) return;
      showFault(faultFrom(error));
    });
    engine = activeEngine;
    window.profileCamera = limit => activeEngine.profile(limit);
    activeEngine.metrics(metrics => {
      if (current !== generation) return;
      window.cameraPerformance = metrics;
      $('camera-speed').textContent = `${metrics.speed.toFixed(2)}× · ${metrics.virtualSeconds.toFixed(1)} s camera / ${metrics.wallSeconds.toFixed(1)} s wall`;
    });
    activeEngine.frames(frame => {
      if (current !== generation) return;
      viewer.frame(frame);
      if (!hasModel) {
        const canvas = $<HTMLCanvasElement>('flat-lcd'); canvas.hidden = false; canvas.width = frame.width; canvas.height = frame.height;
        const context = canvas.getContext('2d')!, rgba = context.createImageData(frame.width, frame.height);
        for (let s = 0, t = 0; s < frame.rgb.length; s += 3, t += 4) { rgba.data[t] = frame.rgb[s]; rgba.data[t + 1] = frame.rgb[s + 1]; rgba.data[t + 2] = frame.rgb[s + 2]; rgba.data[t + 3] = 255; } context.putImageData(rgba, 0, 0);
      }
    });
    const started = performance.now();
    const inputs = await activeEngine.boot(bytes, { ...entry.emulate, initialPositions: device?.initialPositions, restart }, (message, percent) => {
      if (current !== generation) return;
      status(message); const progress = $<HTMLProgressElement>('boot-progress'); progress.hidden = false; progress.value = percent;
    });
    await modelReady;
    if (current !== generation) return;
    $('flat-lcd').hidden = hasModel;
    panel.render(inputs, activeEngine, hasModel ? device?.model : undefined, device?.initialPositions);
    cameraReady = true; cardBrowser.connect(activeEngine); $<HTMLButtonElement>('try-sd').disabled = false;
    sensor.connect(activeEngine); $('panel').hidden = false; $('scene-controls').hidden = false;
    $('live-label').textContent = 'LIVE'; $('boot-progress').hidden = true;
    $('viewer').dataset.bootSeconds = ((performance.now() - started) / 1000).toFixed(3);
    status('Camera ready');
  } catch (error) { if (current === generation) { if (!(error instanceof EngineError)) status(String(error), true); $('boot-progress').hidden = true; } }
}
function firmwareRow(entry: Firmware) {
  const article = document.createElement('article'); article.className = 'firmware';
  const header = document.createElement('div'); header.className = 'firmware-heading';
  const name = document.createElement('strong'); name.textContent = `${entry.model} · ${entry.version}`;
  const link = document.createElement('a'); link.textContent = 'download from Fujifilm'; link.href = entry.download_url; link.target = '_blank'; link.rel = 'noopener noreferrer';
  header.append(name, link); const info = document.createElement('p');
  const capabilities = [entry.emulate && 'Emulator', release?.patches[entry.sha256] && 'SD patch', !entry.emulate && !release?.patches[entry.sha256] && 'Unsupported'].filter(Boolean);
  info.textContent = capabilities.join(' · ') + (entry.release_date ? ` · ${entry.release_date}` : '');
  const hash = document.createElement('code'); hash.textContent = `sha256: ${entry.sha256}`; article.append(header, info, hash); $('firmwares').append(article);
}
function addDownload(blob: Blob, filename: string, text: string, secondary = false) {
  const link = document.createElement('a'), url = URL.createObjectURL(blob); downloadUrls.push(url);
  link.href = url; link.download = filename; link.textContent = text; if (secondary) link.className = 'secondary'; $('downloads').append(link); return link;
}
$('build-patch').onclick = () => {
  if (!selectedFile || !release || !selectedFirmware || patchWorker) return;
  const worker = new Worker(new URL('./patch/worker.ts', import.meta.url), { type: 'module' }); patchWorker = worker;
  const button = $<HTMLButtonElement>('build-patch'); button.disabled = true;
  worker.onmessage = ({ data }) => {
    if (patchWorker !== worker) return;
    if (data.stage) { $('patch-status').textContent = data.stage; return; }
    worker.terminate(); patchWorker = undefined; button.disabled = false;
    if (data.error || data.unsupported) { $('patch-status').textContent = data.error || 'Unsupported patch.'; return; }
    const { result, firmware } = data;
    patchSeeds = ['ffboot','ffsys'].map(name => ({ path: `/${name}.bin`, bytes: new Uint8Array(result[name]) }));
    $('try-sd').hidden = !engine; $<HTMLButtonElement>('try-sd').disabled = !cameraReady;
    $('downloads').replaceChildren(); for (const url of downloadUrls) URL.revokeObjectURL(url); downloadUrls = [];
    const archive = addDownload(cardZip(result), `fujimod-${firmware.id}-sd.zip`, 'Download SD boot files (.zip)');
    for (const name of ['ffboot','ffsys']) addDownload(new Blob([result[name]], { type: 'application/octet-stream' }), `${name}.bin`, `${name}.bin`, true);
    $('result-mods').textContent = `Included mods: ${result.metadata.mods.join(', ')}`;
    $('boot-instructions').replaceChildren(); for (const step of firmware.boot_instructions) { const li = document.createElement('li'); li.textContent = step; $('boot-instructions').append(li); }
    $('patch-status').textContent = ''; $('result').hidden = false; archive.click();
  };
  worker.onerror = event => { $('patch-status').textContent = event.message; worker.terminate(); patchWorker = undefined; button.disabled = false; };
  worker.postMessage({ file: selectedFile, index: release, artifactBase: new URL('artifacts/', document.baseURI).href });
};
$('try-sd').onclick = async () => {
  if (!engine || !cameraReady || !patchSeeds.length) return;
  const active = engine, current = generation, button = $<HTMLButtonElement>('try-sd');
  button.disabled = true; cameraReady = false; panel.disable(); sensor.connect(); $('live-label').textContent = 'RESTARTING';
  try {
    await active.restartWithCardFiles(patchSeeds, (message, percent) => {
      if (current !== generation) return;
      status(message); const progress = $<HTMLProgressElement>('boot-progress'); progress.hidden = false; progress.value = percent;
    });
    if (current !== generation) return;
    cameraReady = true; panel.resume(); sensor.connect(active); cardBrowser.connect(active);
    $('boot-progress').hidden = true; $('live-label').textContent = 'LIVE'; status('Camera ready · SD boot files inserted');
  } catch (error) { if (current === generation && !(error instanceof EngineError)) status('Could not restart with SD files.', true); }
  finally { if (current === generation) button.disabled = !cameraReady; }
};
$<HTMLInputElement>('firmware-file').onchange = event => { const input = event.target as HTMLInputElement; void processFile(input.files?.[0]); input.value = ''; };
$('viewer').addEventListener('dragover', event => { event.preventDefault(); $('viewer').classList.add('dragging'); });
$('viewer').addEventListener('dragleave', () => $('viewer').classList.remove('dragging'));
$('viewer').addEventListener('drop', event => { event.preventDefault(); $('viewer').classList.remove('dragging'); if (event.dataTransfer?.files.length !== 1) { status('Drop one firmware DAT.', true); return; } void processFile(event.dataTransfer.files[0]); });
$('face-screen').onclick = () => viewer.faceScreen(); $('face-lens').onclick = () => viewer.faceLens();
$<HTMLInputElement>('image-file').onchange = event => { const input = event.target as HTMLInputElement; if (input.files?.[0]) { $<HTMLInputElement>('webcam').checked = false; void sensor.useImage(input.files[0]).catch(error => status(String(error), true)); } input.value = ''; };
$('image-drop').ondragover = event => { event.preventDefault(); $('image-drop').classList.add('over'); };
$('image-drop').ondragleave = () => $('image-drop').classList.remove('over');
$('image-drop').ondrop = event => { event.preventDefault(); event.stopPropagation(); $('image-drop').classList.remove('over'); if (event.dataTransfer?.files[0]) { $<HTMLInputElement>('webcam').checked = false; void sensor.useImage(event.dataTransfer.files[0]).catch(error => status(String(error), true)); } };
$<HTMLInputElement>('webcam').onchange = async event => { const input = event.target as HTMLInputElement; try { await sensor.webcam(input.checked); } catch (error) { input.checked = false; status(String(error), true); } };
window.addEventListener('beforeunload', () => { sensor.dispose(); engine?.dispose(); });
try {
  const response = await fetch(new URL(__FUJIMOD_CATALOG__, document.baseURI));
  if (!response.ok) throw Error('Firmware catalog unavailable.');
  catalog = validateCatalog(await response.json());
  const patchResponse = await fetch(new URL('artifacts/patches.json', document.baseURI));
  if (patchResponse.ok && patchResponse.headers.get('content-type')?.includes('json')) { release = validateIndex(await patchResponse.json()); $('release-label').textContent = release.release; }
  const entries = [...catalog.firmwares];
  for (const entry of release?.firmwares ?? []) if (!entries.some(known => known.sha256 === entry.sha256)) entries.push(entry);
  entries.forEach(firmwareRow);
  if (__FUJIMOD_LOCAL_DAT__) {
    const dat = await fetch('/local/firmware.dat'); if (dat.ok) await processFile(new File([await dat.blob()], 'FPUPDATE.DAT'));
  }
} catch (error) { status(String(error), true); }
