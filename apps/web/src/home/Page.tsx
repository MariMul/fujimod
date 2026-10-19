import { CameraLibrary } from '../cameras/Library';
import { For, Show, createSignal, onSettled } from 'solid-js';
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
import { rendererBackend, rendererURL } from '../viewer/renderer';
import '../style.css';
import '../navigation.css';

export default function Home() {
  const [state, setState] = createHomeState();
  const [runtime, setRuntime] = createSignal<HomeRuntime>();
  const [dragging, setDragging] = createSignal(false);
  const [feedback, setFeedback] = createSignal<{ event: 'feedback' | 'crash' }>();
  const [backend, setBackend] = createSignal('webgl2'),
    [gpu, setGpu] = createSignal(false);
  const choice = createFirmwareChoice();
  let viewer!: HTMLDivElement,
    workspace!: HTMLElement,
    controls!: HTMLDivElement,
    file!: HTMLInputElement,
    canvas!: HTMLCanvasElement,
    lcd!: HTMLCanvasElement;
  onSettled(() => {
    const app = startHome(
      state,
      setState,
      { viewer, workspace, controls, file, canvas, lcd },
      choice,
    );
    setRuntime(app);
    setBackend(rendererBackend(location.search));
    setGpu('gpu' in navigator);
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
          <section ref={workspace} class={['stage', { idle: state.idle }]} id="workspace">
            <div
              ref={viewer}
              class={['viewer', { dragging: dragging(), 'lcd-mode': state.view === 'lcd' }]}
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
                ref={canvas}
                id="simulation-canvas"
                class="model-canvas"
                tabindex="0"
                aria-label="Camera environment"
              />
              <canvas
                ref={lcd}
                id="lcd-source"
                hidden={state.view !== 'lcd'}
                class={{ flat: state.view === 'lcd' }}
                width="720"
                height="480"
              />
              <ViewerOverlays viewer={runtime()?.viewer} />
              <div class="ghost" aria-hidden="true" />
              <EmulatorFeedback runtime={runtime()} request={feedback()}>
                {(actions) => (
                  <ViewportControls
                    active={state.toolbarVisible}
                    view={state.view}
                    hasModel={state.hasModel}
                    title={
                      state.firmware ? `${state.firmware.model} · ${state.firmware.version}` : ''
                    }
                    speed={state.speed}
                    mode={runtime()?.viewer.ui.mode ?? 'inspect'}
                    error={runtime()?.viewer.ui.error}
                    modSupported={state.modSupported}
                    modEnabled={state.modEnabled}
                    modDisabled={state.modDisabled}
                    backend={backend()}
                    gpu={gpu()}
                    feedback={actions}
                    setView={(view) => runtime()?.setView(view)}
                    face={(face) =>
                      face === 'screen'
                        ? runtime()?.viewer.faceScreen()
                        : runtime()?.viewer.faceLens()
                    }
                    toggleMode={() => runtime()?.viewer.toggleMode()}
                    selectMod={(enabled) => void runtime()?.selectMod(enabled)}
                    selectRenderer={(value) => location.assign(rendererURL(location.href, value))}
                  />
                )}
              </EmulatorFeedback>
              <label
                class="dropbox"
                id="firmware-prompt"
                for="firmware-file"
                hidden={!state.promptVisible}
              >
                <strong>Drop a firmware DAT</strong>
                <span>or click to choose it</span>
              </label>
              <input
                ref={file}
                class="visually-hidden"
                id="firmware-file"
                type="file"
                accept=".dat,.DAT"
                aria-label="Choose firmware DAT"
                onChange={(event) => {
                  runtime()?.selectFile(event.currentTarget.files?.[0]);
                  event.currentTarget.value = '';
                }}
              />
              <progress
                id="boot-progress"
                class="visually-hidden"
                max="100"
                value={state.progress ?? 0}
                hidden={state.progress === undefined}
                aria-label="It takes a bit to boot a camera in the browser."
              />
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
              <div ref={controls} id="controls">
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
            <svg id="leaders" viewBox={runtime()?.viewer.ui.viewBox || '0 0 1 1'}>
              <For each={runtime()?.viewer.ui.paths ?? []}>
                {(path) => (
                  <path
                    data-input={path.id}
                    data-x={path.x}
                    data-y={path.y}
                    d={path.d}
                    class={{ active: path.active }}
                  />
                )}
              </For>
            </svg>
          </section>
          <CameraTable
            columns={generations}
            cameras={state.cameras}
            emulate={(firmware) => runtime()?.selectFirmware(firmware)}
          />
        </main>
        <footer id="model-credit" hidden={!state.creditVisible}>
          <Show when={state.credit}>
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
