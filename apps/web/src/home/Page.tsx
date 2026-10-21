import { LoadingAssets } from './LoadingAssets';
import { CameraLibrary } from '../cameras/Library';
import { Show, createSignal, onSettled } from 'solid-js';
import { createHomeState } from './state';
import { startHome, type HomeRuntime } from './runtime';
import { Controls } from '../ui/controls';
import { CardBrowser } from '../ui/card';
import { Connections } from '../ui/connections';
import { FaultPanel } from '../ui/fault';
import { CameraTable } from '../ui/cameras';
import { PersonalNote } from '../ui/personal-note';
import { FeatureCatalog } from './Features';
import { FirmwareChoice, createFirmwareChoice } from '../ui/firmware-actions';
import { EmulatorFeedback } from './Feedback';
import { ViewportControls } from './ViewportControls';
import generations from '../../devices/generations.json';
import { ViewerOverlays } from '../viewer/Overlays';
import '../style.css';
import '../navigation.css';
import './loading.css';

export default function Home() {
  const [state, setState] = createHomeState();
  const [runtime, setRuntime] = createSignal<HomeRuntime>();
  const [dragging, setDragging] = createSignal(false);
  const [feedback, setFeedback] = createSignal<{ event: 'feedback' | 'crash' }>();
  const choice = createFirmwareChoice();
  let viewer!: HTMLDivElement,
    workspace!: HTMLElement,
    file!: HTMLInputElement,
    lcd!: HTMLCanvasElement;
  onSettled(() => {
    const app = startHome(state, setState, { viewer, workspace, file, lcd }, choice);
    setRuntime(app);
    if (new URLSearchParams(location.search).has('feedback')) setFeedback({ event: 'feedback' });
    const dragOver = (event: DragEvent) => {
      if (event.dataTransfer?.types.includes('Files')) event.preventDefault();
    };
    const drop = (event: DragEvent) => {
      if (event.defaultPrevented || !event.dataTransfer?.files[0]) return;
      event.preventDefault();
      app.selectFile(event.dataTransfer.files[0]);
    };
    window.addEventListener('dragover', dragOver);
    window.addEventListener('drop', drop);
    return () => {
      window.removeEventListener('dragover', dragOver);
      window.removeEventListener('drop', drop);
      app.dispose();
    };
  });
  return (
    <>
      <div class="shell">
        <header>
          <a href="./" aria-label="Fujimod">
            <img src="brand.png" alt="Fujimod" width="110" height="28" />
          </a>
          <nav class="site-nav" aria-label="Pages">
            <a href="dev-tools/">Dev Tools</a>
          </nav>
          <div class="meta build-info">
            <a id="release-label" href={state.buildURL}>
              {state.buildLabel}
            </a>
            <a href="https://github.com/justjake/fujimod">github.com/justjake/fujimod</a>
          </div>
        </header>
        <main>
          <Show when={!state.activeCamera && state.savedCameras.length === 0}>
            <PersonalNote />
            <FeatureCatalog />
          </Show>
          <h2 class="section-heading" id="emulator-heading">
            Emulator
          </h2>
          <CameraLibrary
            cameras={state.savedCameras}
            active={state.activeCamera}
            busy={state.cameraBusy}
            message={state.storageMessage}
            error={state.storageError}
            open={(camera) => {
              void runtime()?.openCamera(camera);
            }}
            close={() => {
              void runtime()?.closeCamera();
            }}
            upload={() => file.click()}
          />
          <section
            ref={workspace}
            class={['stage', { idle: state.idle }]}
            id="workspace"
            aria-labelledby="emulator-heading"
          >
            <div
              ref={viewer}
              class={['viewer lcd-mode', { dragging: dragging() }]}
              id="viewer"
              aria-label="Camera viewer"
              style={{ '--body-preview': state.bodyPreview }}
              onDragOver={(event) => {
                event.preventDefault();
                setDragging(true);
              }}
              onDragLeave={() => setDragging(false)}
              onDrop={(event) => {
                event.preventDefault();
                setDragging(false);
                runtime()?.selectFile(event.dataTransfer?.files[0]);
              }}
            >
              <canvas
                ref={lcd}
                id="lcd-source"
                class="flat"
                aria-label="Camera LCD"
                width="720"
                height="480"
              />
              <ViewerOverlays viewer={runtime()?.viewer} />
              <div class="ghost" aria-hidden="true" />
              <EmulatorFeedback runtime={runtime()} request={feedback()}>
                {(actions) => (
                  <ViewportControls
                    active={state.toolbarVisible}
                    title={
                      state.firmware ? `${state.firmware.model} · ${state.firmware.version}` : ''
                    }
                    speed={state.speed}
                    webcamSupported={!!state.firmware?.emulate?.sceneInput}
                    webcamActive={state.webcamActive}
                    webcamBusy={state.webcamBusy}
                    webcamDisabled={state.loading || !!state.fault}
                    webcamError={state.webcamError}
                    toggleWebcam={() => runtime()?.toggleWebcam()}
                    modSupported={state.modSupported}
                    modEnabled={state.modEnabled}
                    modDisabled={state.modDisabled}
                    feedback={actions}
                    selectMod={(enabled) => void runtime()?.selectMod(enabled)}
                  />
                )}
              </EmulatorFeedback>
              <label
                class="dropbox"
                id="firmware-prompt"
                for="firmware-file"
                hidden={!state.promptVisible}
              >
                <strong>drop FPUPDATE.DAT firmware file</strong>
                <span>you can download it from Fujifilm</span>
              </label>
              <input
                ref={file}
                class="visually-hidden"
                id="firmware-file"
                type="file"
                accept=".dat,.DAT"
                aria-label="Choose FPUPDATE.DAT"
                onChange={(event) => {
                  runtime()?.selectFile(event.currentTarget.files?.[0]);
                  event.currentTarget.value = '';
                }}
              />
              <div id="camera-loading" class="camera-loading" hidden={!state.loading}>
                <Show
                  when={
                    !state.loadingAssets.some(
                      (asset) => asset.status === 'loading' || asset.status === 'waiting',
                    )
                  }
                >
                  <p>{state.message}</p>
                </Show>
                <LoadingAssets assets={state.loadingAssets} />
                <progress
                  id="boot-progress"
                  hidden={state.loadingAssets.some(
                    (asset) => asset.status === 'loading' || asset.status === 'waiting',
                  )}
                  max="100"
                  value={state.progress}
                  aria-label={state.message || 'Loading camera'}
                />
              </div>
              <span id="status" class={{ error: state.error }} role="status" aria-live="polite">
                {state.message}
              </span>
              <span id="live-label">{state.live}</span>
            </div>
            <aside
              class="panel"
              id="panel"
              aria-label="Camera controls"
              hidden={!state.panelVisible}
            >
              <div id="controls">
                <Controls session={runtime()?.panel} />
              </div>
              <hr />
              <CardBrowser
                engine={runtime()?.cardEngine()}
                present={(value) => runtime()?.insertCard(value) ?? Promise.resolve()}
                format={() => runtime()?.formatCard() ?? Promise.resolve()}
              />
              <Connections rows={state.connections} />
            </aside>
          </section>
          <CameraTable
            columns={generations}
            cameras={state.cameras}
            emulate={(firmware) => runtime()?.selectFirmware(firmware)}
          />
        </main>
        <footer id="preview-credit" hidden={!state.idle}>
          <Show when={state.previewCredit}>
            {(credit) => (
              <>
                "<a href={credit().url}>{credit().title}</a>" by{' '}
                <a href={credit().authorUrl}>{credit().author}</a> ·{' '}
                <a href={credit().license}>CC BY 4.0</a>
              </>
            )}
          </Show>
        </footer>
      </div>
      <FaultPanel
        fault={state.fault}
        context={state.faultContext}
        canUndo={state.canUndo}
        undoBusy={state.undoBusy}
        canRestart={state.canRestart}
        undo={() => void runtime()?.undo()}
        restart={() => void runtime()?.restart()}
        report={() => setFeedback({ event: 'crash' })}
      />
      <FirmwareChoice choice={choice} />
    </>
  );
}
