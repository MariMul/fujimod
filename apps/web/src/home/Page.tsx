import { EmulatorWorkspace } from '@fujimod/emulator-ui/EmulatorWorkspace';
import { CameraLibrary } from '../cameras/Library';
import { Show, createSignal, onSettled } from 'solid-js';
import { createHomeState } from './state';
import { startHome, type HomeController } from './controller';
import { FaultPanel } from '@fujimod/emulator-ui/ui/fault';
import { CameraTable } from '../ui/cameras';
import { PersonalNote } from '../ui/personal-note';
import { FeatureCatalog } from './Features';
import { FirmwareChoice, createFirmwareChoice } from '../ui/firmware-actions';
import { EmulatorFeedback } from '@fujimod/emulator-ui/Feedback';
import { ViewportControls } from '@fujimod/emulator-ui/ViewportControls';
import generations from '../../devices/generations.json';
import '../home/site.css';
import '../navigation.css';

export default function Home() {
  const [state, setState] = createHomeState();
  const [runtime, setRuntime] = createSignal<HomeController>();
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
          <EmulatorWorkspace
            state={state}
            runtime={runtime()}
            bodyPreview={state.bodyPreview}
            labelledBy="emulator-heading"
            workspace={(element) => {
              workspace = element;
            }}
            viewer={(element) => {
              viewer = element;
            }}
            lcd={(element) => {
              lcd = element;
            }}
            selectFile={(selected) => runtime()?.selectFile(selected)}
            toolbar={
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
            }
          >
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
          </EmulatorWorkspace>
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
