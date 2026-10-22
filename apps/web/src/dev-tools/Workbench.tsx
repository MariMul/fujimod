import {
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onSettled,
  untrack,
} from 'solid-js';
import type { CameraModel } from './camera-model';
import { CameraDetails } from './CameraDetails';
import { CameraStrip } from './CameraStrip';
import { CameraTools, Wizard, emptyReadings, type Readings } from './CameraTools';
import { ConsoleLog } from './ConsoleLog';
import { ToolDeck, type SavedCamera } from './ToolDeck';
import { CameraStore } from '../cameras/store';
import { DebugModeWizard, WizardConnection } from './debug-mode/Wizard';
import { LeaveDebugMode } from './debug-mode/LeaveDebugMode';
import { debugModeOperations } from './debug-mode/operations';
import { createOwnerCountdown, type Countdown } from './owner-countdown';
import { StateCaptures } from './captures/StateCaptures';
import { MapControls } from './captures/MapControls';
import type { CaptureSet } from './captures/capture';
import './workbench.css';

/** Dev Tools: the splash until a camera is known, then the workbench. */
export function Workbench(props: {
  model: CameraModel;
  usbSupported: boolean;
  initialWizard?: Wizard;
}) {
  const [selected, setSelected] = createSignal<string>();
  const [wizard, setWizard] = createSignal(props.initialWizard);
  const [connecting, setConnecting] = createSignal(false);
  const [connectError, setConnectError] = createSignal('');
  const [readings, setReadings] = createSignal<ReadonlyMap<string, Readings>>(new Map());
  const [captures, setCaptures] = createSignal<ReadonlyMap<string, CaptureSet>>(new Map());
  const [countdown, setCountdown] = createSignal<Countdown>();
  const owner = createOwnerCountdown(setCountdown);
  onCleanup(
    untrack(() => props.model.onOwnerCountdown((_camera, message) => owner.start(message))),
  );
  onCleanup(() => owner.cancel());

  const [savedCameras, setSavedCameras] = createSignal<readonly SavedCamera[]>();
  // The emulator's saved cameras, offered as simulated cameras.
  onSettled(() => {
    new CameraStore().list().then(
      (records) => setSavedCameras(records.map(({ id, name }) => ({ id, name }))),
      () => setSavedCameras([]),
    );
  });
  const cameras = () => props.model.cameras();
  // Follow the newest camera unless one is chosen; keep a chosen camera even once unplugged.
  createEffect(
    () => cameras().map((camera) => camera.id),
    (ids) => {
      const current = selected();
      if (!current || !ids.includes(current)) setSelected(ids.at(-1));
    },
  );
  const camera = createMemo(() => cameras().find((entry) => entry.id === selected()));

  async function connect() {
    if (connecting()) return;
    setConnecting(true);
    setConnectError('');
    const known = new Set(cameras().map((entry) => entry.id));
    try {
      await props.model.connectUsb();
      const added = cameras().find((entry) => !known.has(entry.id));
      if (added) setSelected(added.id);
    } catch (error) {
      setConnectError(error instanceof Error ? error.message : String(error));
    } finally {
      setConnecting(false);
    }
  }
  async function connectSimulated(savedCamera: string) {
    const start = props.model.connectSimulated;
    if (!start || connecting()) return;
    setConnecting(true);
    setConnectError('');
    try {
      await start.call(props.model, savedCamera);
    } catch (error) {
      setConnectError(error instanceof Error ? error.message : String(error));
    } finally {
      setConnecting(false);
    }
  }
  const wizardConnection = () => {
    if (!props.usbSupported) return WizardConnection.Unsupported;
    if (connecting()) return WizardConnection.Choosing;
    const current = camera();
    return current?.present && current.transport === 'usb'
      ? WizardConnection.Connected
      : WizardConnection.Disconnected;
  };
  // Depend on the camera's id only: a session changes the camera's fields, and new
  // operations would restart the wizard's reset-guard step.
  const usbCamera = createMemo(() => {
    const current = camera();
    return current?.transport === 'usb' ? current.id : undefined;
  });
  const operations = createMemo(() => {
    const id = usbCamera();
    if (!id) return undefined;
    return debugModeOperations((tool, operation) =>
      props.model.withSession(id, tool, (session) => operation(session.debug)),
    );
  });
  const cameraReadings = () => readings().get(selected() ?? '') ?? emptyReadings();
  function updateReadings(update: (current: Readings) => Readings) {
    const id = selected();
    if (!id) return;
    setReadings((map) => new Map(map).set(id, update(map.get(id) ?? emptyReadings())));
  }
  const closeWizard = () => setWizard(undefined);
  const cameraCaptures = () => captures().get(selected() ?? '');
  function updateCaptures(set: CaptureSet) {
    const id = selected();
    if (id) setCaptures((map) => new Map(map).set(id, set));
  }

  return (
    <div class="wb">
      <Show when={countdown()}>
        {(state) => (
          <div class="wb-countdown" role="status" aria-live="polite">
            <span>{state().message}</span>
            <strong>{state().remaining}</strong>
          </div>
        )}
      </Show>
      <Show
        when={cameras().length || wizard()}
        fallback={
          <ToolDeck
            busy={connecting()}
            usbSupported={props.usbSupported}
            onServiceMode={() => setWizard(Wizard.Service)}
            onConnect={() => void connect()}
            savedCameras={savedCameras()}
            simulationAvailable={Boolean(props.model.connectSimulated)}
            error={connectError()}
            onSimulated={(savedCamera) => void connectSimulated(savedCamera)}
          />
        }
      >
        <Show when={cameras().length}>
          <CameraStrip
            cameras={cameras()}
            selected={selected()}
            connecting={connecting()}
            onSelect={setSelected}
            onConnect={() => void connect()}
          />
        </Show>
        <Show when={camera()?.serviceMode && camera()?.present}>
          <div class="wb-service" role="alert">
            <strong>Service mode</strong>
            <span>
              Switching the camera off in service mode restores factory settings. Remove the battery
              to leave service mode.
            </span>
          </div>
        </Show>
        <div class="wb-bench">
          <aside class="wb-tools" aria-label="Tools">
            <Show when={connectError()}>
              <p class="wb-status" role="alert">
                {connectError()}
              </p>
            </Show>
            <Switch>
              <Match when={wizard() === Wizard.Service}>
                <DebugModeWizard
                  purpose="service"
                  connection={wizardConnection()}
                  onConnect={() => void connect()}
                  onClose={closeWizard}
                />
              </Match>
              <Match when={wizard() === Wizard.Enable}>
                <DebugModeWizard
                  connection={wizardConnection()}
                  onConnect={() => void connect()}
                  operations={operations()}
                  onClose={closeWizard}
                />
              </Match>
              <Match when={wizard() === Wizard.Disable}>
                <LeaveDebugMode
                  connected={wizardConnection() === WizardConnection.Connected}
                  operation={operations()}
                  onClose={closeWizard}
                />
              </Match>
              <Match when={wizard() === Wizard.StateCaptures}>
                <StateCaptures
                  camera={camera()}
                  model={props.model}
                  set={cameraCaptures()}
                  onSet={updateCaptures}
                  onConnect={() => void connect()}
                  onClose={closeWizard}
                />
              </Match>
              <Match when={wizard() === Wizard.MapControls}>
                <MapControls
                  camera={camera()}
                  model={props.model}
                  set={cameraCaptures()}
                  onSet={updateCaptures}
                  onConnect={() => void connect()}
                  onClose={closeWizard}
                />
              </Match>
              <Match when={camera()}>
                {(current) => (
                  <>
                    <CameraDetails camera={current()} model={props.model} />
                    <CameraTools
                      camera={current()}
                      model={props.model}
                      readings={cameraReadings()}
                      onReadings={updateReadings}
                      onWizard={setWizard}
                    />
                  </>
                )}
              </Match>
            </Switch>
            <div class="wb-foot">
              <button class="wb-text" onClick={() => setWizard(Wizard.Service)}>
                Service mode instructions
              </button>
              <a class="wb-text" href="../bug/">
                Bug report
              </a>
            </div>
          </aside>
          <ConsoleLog
            entries={props.model.log()}
            cameras={cameras()}
            onClear={() => props.model.clearLog()}
          />
        </div>
      </Show>
    </div>
  );
}
