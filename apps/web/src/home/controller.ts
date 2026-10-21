import { reconcile } from 'solid-js';
import { startSession } from '@fujimod/emulator-ui/session/controller';
import { DownloadProgress } from '@fujimod/emulator-ui/engine/download-progress';
import { CameraStore, type CameraRecord } from '../cameras/store';
import '../home/site.css';
import { flags } from '@fujimod/emulator-ui/flags';
import { cardZip } from '../patch/download';
import type { PatchResult } from '../patch/patcher';
import { validateCatalog, type Catalog, type Firmware } from '../catalog';
import { validateIndex, verifyPatch } from '../patch/catalog';
import { sha256 } from '../patch/patcher';
import { EngineError, faultFrom } from '@fujimod/emulator-ui/engine/fault';
import { cachedFetch } from '../asset-cache';
import type { CardFile, BootMedia } from '@fujimod/emulator-ui/engine/api';
import type { ReplayScenario } from '../scenarios';
import { DEVICE_PANEL_SCHEMA, type DevicePanel } from '@fujimod/emulator-ui/ui/layout';

import { downloadBlob } from '@fujimod/emulator-ui/ui/download';
import { firmwareActions, type createFirmwareChoice } from '../ui/firmware-actions';
import type { HomeState, PreviewCredit } from './state';
import type { HomeSetter } from './state';

import { supportedCameras } from './state';
declare const __FUJIMOD_LOCAL_DAT__: boolean;
declare const __FUJIMOD_SIMULATOR_BUILD__: string;
declare const __FUJIMOD_CATALOG__: string;
const adapters = import.meta.glob<{
  panel: DevicePanel;
  preview?: string;
  previewCredit?: PreviewCredit;
  sdBoot?: boolean;
}>('../../devices/*/index.ts');
export function startHome(
  state: HomeState,
  setState: HomeSetter,
  refs: {
    viewer: HTMLElement;
    workspace: HTMLElement;
    lcd: HTMLCanvasElement;
    file: HTMLInputElement;
  },
  choice: ReturnType<typeof createFirmwareChoice>,
) {
  performance.mark('camera-module-ready');
  let catalog: Catalog = state.catalog,
    release: ReturnType<typeof validateIndex> | undefined = state.release;
  const cameras = new CameraStore();
  let activeCamera: CameraRecord | undefined;
  let generation = 0,
    selectedFile: (() => Promise<File>) | undefined,
    selectedFirmware: Firmware | undefined;
  let firmwareHash: string | undefined;
  let manualSelection = false;
  let build = 'Development';
  let patchWorker: Worker | undefined;
  let replayScenario: ReplayScenario | undefined;
  let modEnabled = true;
  let modFirmwareHash: string | undefined;
  const session = startSession(state, setState, refs, {
    async createEngine(onError) {
      const { createEngine } = await import('../engine');
      return createEngine(onError);
    },
    identity: () => ({
      camera: selectedFirmware && { id: selectedFirmware.id, model: selectedFirmware.model },
      firmwareSha256: firmwareHash,
      build,
      modBuild: !selectedFirmware
        ? 'Not identified'
        : !modEnabled
          ? 'Unmodded'
          : release?.patches[selectedFirmware.sha256]
            ? `${release.release}${release.built_utc ? ` / ${release.built_utc}` : ' / Build date unavailable'}`
            : 'Stock firmware (no mods)',
      canRestart: !!selectedFile,
      sceneInput: !!selectedFirmware?.emulate?.sceneInput,
    }),
    simulatorBuild: __FUJIMOD_SIMULATOR_BUILD__,
    restart,
    fault: () => updateModButtons(!selectedFile),
    ready: () => updateModButtons(false),
    cameraSaved(event) {
      if (event.error) {
        storageStatus(`Camera not saved · ${event.error}`, true);
        return;
      }
      if (activeCamera && event.revision !== undefined) {
        activeCamera = {
          ...activeCamera,
          revision: event.revision,
          savedAt: event.savedAt,
          modEnabled,
        };
        setState((s) => {
          if (s.activeCamera) {
            s.activeCamera.revision = event.revision!;
            s.activeCamera.savedAt = event.savedAt;
            s.activeCamera.modEnabled = modEnabled;
          }
        });
      }
      storageStatus('Saved');
      void refreshCameras().catch((error) => storageStatus(String(error), true));
    },
  });
  const { showFault, status, progress, finishLoading } = session;
  function updateModButtons(disabled: boolean) {
    setState((s) => {
      s.modDisabled = disabled;
      s.modEnabled = modEnabled;
    });
  }
  async function restart() {
    if (!selectedFile) return;
    const reload = selectedFile;
    try {
      await processFile(await reload(), true, undefined, reload, activeCamera);
    } catch (error) {
      showFault(faultFrom(error));
    }
  }
  async function selectMod(enabled: boolean) {
    if (!selectedFile || enabled === modEnabled) return;
    modEnabled = enabled;
    updateModButtons(true);
    await restart();
  }
  async function loadDevice(body: string) {
    const load = adapters[`../../devices/${body}/index.ts`];
    if (!load)
      return {
        panel: { schema: DEVICE_PANEL_SCHEMA, body, layout: [], combos: [] } satisfies DevicePanel,
      };
    const device = await load();
    return device;
  }
  function reset() {
    session.reset();
    choice.cancel();
    updateModButtons(true);
    setState((s) => {
      s.modSupported = false;
      s.firmware = selectedFirmware;
    });
    patchWorker?.terminate();
    patchWorker = undefined;
  }
  function bootMedia(file: File): BootMedia {
    const index = release,
      selected = selectedFirmware!;
    const firmware = index?.firmwares.find((entry) => entry.sha256 === selected.sha256);
    if (!index || !firmware || !index.patches[selected.sha256])
      return { identities: Promise.resolve([]), load: async () => [] };
    const metadata = (async () => {
      const response = await fetch(
        new URL(`artifacts/${index.patches[selected.sha256]}`, document.baseURI),
      );
      if (!response.ok) throw Error('The release patch could not be downloaded.');
      return verifyPatch(index, firmware, new Uint8Array(await response.arrayBuffer()));
    })();
    return {
      identities: metadata.then((meta) =>
        ['ffboot', 'ffsys'].map((name) => ({
          path: `/${name}.bin`,
          sha256: meta.outputs[name as 'ffboot' | 'ffsys'].sha256,
        })),
      ),
      load: () => patchedFiles(file),
    };
  }
  function patchedFiles(file: File): Promise<CardFile[]> {
    return patchFirmware(file).then((result) =>
      result
        ? ['ffboot', 'ffsys'].map((name) => ({
            path: `/${name}.bin`,
            bytes: result[name as 'ffboot' | 'ffsys'],
          }))
        : [],
    );
  }
  function patchFirmware(file: File): Promise<PatchResult | undefined> {
    if (!release?.patches[selectedFirmware!.sha256]) return Promise.resolve(undefined);
    return new Promise((resolve, reject) => {
      const worker = new Worker(new URL('../patch/worker.ts', import.meta.url), {
        type: 'module',
      });
      patchWorker = worker;
      worker.onmessage = ({ data }) => {
        if (data.stage) return;
        worker.terminate();
        if (patchWorker === worker) patchWorker = undefined;
        if (data.error) return reject(Error(data.error));
        if (data.unsupported) return resolve(undefined);
        resolve(data.result);
      };
      worker.onerror = (error) => {
        worker.terminate();
        reject(Error(error.message));
      };
      worker.postMessage({
        file,
        index: release,
        artifactBase: new URL('artifacts/', document.baseURI).href,
      });
    });
  }
  async function processFile(
    file?: File,
    restart = false,
    preparedBytes?: ArrayBuffer,
    reload?: () => Promise<File>,
    camera?: CameraRecord,
  ) {
    if (disposed || !file) return;
    performance.mark('camera-file-received');
    // Finish the previous camera's durable write before releasing its worker.
    if (activeCamera && !session.hasFault()) {
      try {
        await session.saveCamera();
      } catch (error) {
        storageStatus(String(error), true);
        return;
      }
    }
    if (camera?.id === activeCamera?.id) camera = activeCamera;
    const current = ++generation;
    const firmwareAssets = preparedBytes ? [...state.loadingAssets] : [];
    reset();
    setState((s) => {
      s.loadingAssets = firmwareAssets;
    });
    activeCamera = camera;
    storageStatus('');
    setState((s) => {
      if (s.activeCamera && camera) reconcile(camera)(s.activeCamera);
      else s.activeCamera = camera;
    });
    selectedFile = undefined;
    selectedFirmware = undefined;
    firmwareHash = undefined;
    progress('Checking firmware…');
    try {
      if (!file.size || file.size > 256 * 1024 * 1024)
        throw Error('Choose a firmware DAT smaller than 256 MB.');
      const bytes = preparedBytes ?? (await file.arrayBuffer()),
        hash = await sha256(new Uint8Array(bytes));
      if (current !== generation) return;
      performance.mark('camera-dat-verified');
      firmwareHash = hash;
      const entry: Firmware | undefined =
        catalog.firmwares.find((item) => item.sha256 === hash) ??
        (release?.firmwares.find((item) => item.sha256 === hash) as Firmware | undefined);
      if (!entry) throw Error(`Unsupported firmware · SHA-256 ${hash}`);
      if (file.size !== entry.bytes) throw Error('Firmware size differs from the catalog.');
      selectedFile = reload ?? (() => Promise.resolve(file));
      if (replayScenario && entry.id !== replayScenario.firmwareId)
        throw Error('The replay scenario requires firmware ' + replayScenario.firmwareId);
      selectedFirmware = entry;
      if (modFirmwareHash !== hash) {
        modFirmwareHash = hash;
        modEnabled = !replayScenario;
      }
      const device = entry.device ?? (entry.body ? await loadDevice(entry.body) : undefined);
      if (current !== generation) return;
      if (
        device &&
        (device.panel.schema !== DEVICE_PANEL_SCHEMA || device.panel.body !== entry.body)
      )
        throw Error('The emulator control layout does not match this firmware body or schema.');
      if (!restart && !reload && !camera) {
        const patchEntry = release?.firmwares.find((item) => item.sha256 === hash);
        let fpUpdate = false;
        if (flags.allowFpUpdateDownload && patchEntry && release) {
          const response = await fetch(
            new URL(`artifacts/${release.patches[hash]}`, document.baseURI),
          );
          if (!response.ok) throw Error('The release patch could not be downloaded.');
          const metadata = await verifyPatch(
            release,
            patchEntry,
            new Uint8Array(await response.arrayBuffer()),
          );
          fpUpdate = !!metadata.outputs.fpupdate;
        }
        if (current !== generation) return;
        finishLoading();
        const action = await choice.choose(
          firmwareActions(flags, {
            fpUpdate,
            sdCard: !!release?.patches[hash],
          }),
          `${entry.model} · ${entry.version}`,
        );
        if (current !== generation) return;
        if (action !== 'launch') {
          if (action) {
            progress('Building download…', 0);
            const result = await patchFirmware(file);
            if (current !== generation) return;
            if (!result) throw Error('No release patch is available for this firmware.');
            if (action === 'sd-card') downloadBlob(cardZip(result), 'sd-card.zip');
            else {
              if (!flags.allowFpUpdateDownload || !result.fpupdate)
                throw Error('FP update download unavailable.');
              downloadBlob(new Blob([result.fpupdate]), 'FPUPDATE.DAT');
            }
          }
          finishLoading();
          setState((s) => {
            s.promptVisible = true;
          });
          status(action ? 'Download ready' : '');
          return;
        }
      }
      if (!entry.emulate || !device) throw Error('Emulation unsupported for this firmware.');
      progress('Loading emulator…');
      if (!replayScenario && !activeCamera) {
        activeCamera = await cameras.create(file, entry, modEnabled);
        if (current !== generation) return;
        setState((s) => {
          s.activeCamera = activeCamera;
        });
        await refreshCameras();
        // A browser may decline this request; quota errors are still surfaced on every write.
        void navigator.storage?.persist?.().catch(() => {});
      }
      if (camera && !restart) modEnabled = camera.modEnabled;
      setState((s) => {
        s.idle = false;
      });
      setState((s) => {
        s.panelVisible = true;
      });
      setState((s) => {
        s.toolbarVisible = true;
      });
      setState((s) => {
        s.promptVisible = false;
      });
      setState((s) => {
        s.camerasVisible = false;
      });
      setState((s) => {
        s.firmware = selectedFirmware;
      });
      const emulate = entry.emulate;
      await session.boot(
        bytes,
        device.panel,
        () => !disposed && current === generation,
        () => {
          const modSupported =
            'sdBoot' in device && !!device.sdBoot && !!release?.patches[entry.sha256];
          updateModButtons(true);
          setState((s) => {
            s.modSupported = modSupported;
          });
          const bootInputs =
            modSupported && modEnabled
              ? (device.panel.combos.find((combo) => combo.boot === 'sd')?.inputs ?? [])
              : [];
          const media = modSupported && modEnabled ? bootMedia(file) : undefined;
          return {
            config: {
              ...emulate,
              firmwareId: entry.id,
              restart,
              camera: activeCamera && {
                id: activeCamera.id,
                revision: activeCamera.revision,
                modEnabled,
              },
              bootInputs,
              execution: replayScenario
                ? {
                    mode: replayScenario.interactive ? 'live' : 'replay',
                    clockSeconds2000: replayScenario.clockSeconds2000,
                  }
                : undefined,
            },
            media,
          };
        },
      );
      if (current !== generation) return;
      if (replayScenario) {
        refs.workspace.scrollIntoView({ block: 'start', behavior: 'instant' });
        if (!replayScenario.interactive) {
          await session.replay(replayScenario.steps);
        }
      }
    } catch (error) {
      if (current === generation && !(error instanceof EngineError)) showFault(faultFrom(error));
    }
  }
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
    refs.file.click();
  }
  function storageStatus(message: string, error = false) {
    setState((s) => {
      s.storageMessage = message;
      s.storageError = error;
    });
  }
  async function refreshCameras() {
    const records = await cameras.list();
    if (!disposed)
      setState((s) => {
        reconcile(records, 'id')(s.savedCameras);
      });
  }
  async function openCamera(record: CameraRecord) {
    manualSelection = true;
    if (state.cameraBusy) return;
    setState((s) => {
      s.cameraBusy = true;
    });
    try {
      const current = (await cameras.list()).find((camera) => camera.id === record.id);
      if (!current) throw Error('This camera is no longer in browser storage.');
      const blob = await cameras.firmware(record.id);
      await processFile(new File([blob], current.fileName), false, undefined, undefined, current);
    } catch (error) {
      storageStatus(String(error), true);
    } finally {
      setState((s) => {
        s.cameraBusy = false;
      });
    }
  }
  async function closeCamera() {
    if (state.cameraBusy) return;
    setState((s) => {
      s.cameraBusy = true;
    });
    try {
      if (activeCamera && !session.hasFault()) await session.saveCamera();
      ++generation;
      reset();
      activeCamera = undefined;
      selectedFile = undefined;
      selectedFirmware = undefined;
      setState((s) => {
        s.activeCamera = undefined;
      });
      await refreshCameras();
    } catch (error) {
      storageStatus(String(error), true);
    } finally {
      setState((s) => {
        s.cameraBusy = false;
      });
    }
  }
  async function initialize() {
    try {
      await refreshCameras();
    } catch (error) {
      storageStatus(`Browser storage unavailable · ${String(error)}`, true);
    }
    try {
      if (import.meta.env.DEV && new URLSearchParams(location.search).has('scenario')) {
        const { scenarioFromUrl } = await import('../scenarios');
        replayScenario = scenarioFromUrl(location.search);
        if (!__FUJIMOD_LOCAL_DAT__)
          throw Error('Replay scenarios require a configured local firmware DAT.');
      }
      const response = await fetch(new URL(__FUJIMOD_CATALOG__, document.baseURI));
      if (!response.ok) throw Error('Firmware catalog unavailable.');
      if (disposed) return;
      catalog = validateCatalog(await response.json());
      const patchResponse = await fetch(new URL('artifacts/patches.json', document.baseURI));
      if (patchResponse.ok && patchResponse.headers.get('content-type')?.includes('json'))
        release = validateIndex(await patchResponse.json());
      if (disposed) return;
      if (release && !import.meta.env.DEV) {
        build = release.release;
        setState((s) => {
          s.buildLabel = `Build ${build}`;
          s.buildURL = `https://github.com/justjake/fujimod/releases/tag/${encodeURIComponent(build)}`;
        });
      }
      const loadedCameras = supportedCameras(catalog, release);
      if (JSON.stringify(loadedCameras) !== JSON.stringify(state.cameras))
        setState((s) => {
          s.cameras = loadedCameras;
        });
      const cameras = supportedCameras(catalog, release);
      performance.mark('camera-catalog-ready');
      // A configured development DAT must not create a camera on page load.
      // Only an explicit replay URL starts firmware without a user selection.
      const localDat =
        replayScenario && __FUJIMOD_LOCAL_DAT__
          ? new DownloadProgress(progress).fetch(
              new URL(replayScenario.firmwarePath, location.href).href,
              () => cachedFetch(replayScenario!.firmwarePath),
            )
          : undefined;
      const first = cameras.find((camera) => camera.firmware.model_asset);
      if (first?.firmware.model_asset) {
        const device = await loadDevice(first.firmware.model_asset);
        if (device)
          setState((s) => {
            s.bodyPreview = `url(${device.preview})`;
            s.previewCredit = device.previewCredit;
          });
      }
      if (localDat) {
        const dat = await localDat;
        if (dat.ok && !manualSelection) {
          const bytes = await dat.arrayBuffer();
          // The local preview can reread its verified DAT on Restart. Retaining a
          // synthetic File here would keep another entire DAT in browser memory.
          const reload = async () => {
            const response = await cachedFetch(replayScenario!.firmwarePath);
            if (!response.ok) throw Error('The local firmware could not be read.');
            return new File([await response.arrayBuffer()], 'FPUPDATE.DAT');
          };
          if (!manualSelection)
            await processFile(new File([bytes], 'FPUPDATE.DAT'), false, bytes, reload);
        }
      }
    } catch (error) {
      showFault(faultFrom(error));
    }
  }

  let disposed = false;
  const saveHidden = () => {
    if (document.visibilityState === 'hidden') session.stopHiddenWebcam();
    if (document.visibilityState === 'hidden' && activeCamera && !session.hasFault())
      void session
        .saveCamera()
        .catch((error) => storageStatus(`Camera not saved · ${String(error)}`, true));
  };
  document.addEventListener('visibilitychange', saveHidden);
  const unload = session.unload;
  window.addEventListener('beforeunload', unload);
  void initialize();
  return {
    ...session,
    selectMod,
    openCamera,
    closeCamera,
    selectFirmware: (firmware: Firmware) => {
      void tryFirmware(firmware).catch((error) => showFault(faultFrom(error)));
    },
    selectFile: (file?: File) => {
      manualSelection = true;
      void processFile(file);
    },
    dispose() {
      disposed = true;
      ++generation;
      choice.cancel();
      session.dispose();
      patchWorker?.terminate();
      window.removeEventListener('beforeunload', unload);
      document.removeEventListener('visibilitychange', saveHidden);
    },
  };
}
export type HomeController = ReturnType<typeof startHome>;
