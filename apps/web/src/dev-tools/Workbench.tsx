import {
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  untrack,
} from 'solid-js';
import type { CameraModel } from './camera-model';
import { CameraCard, CameraStrip } from './CameraCard';
import { CameraTools, Wizard, emptyReadings, type Readings } from './CameraTools';
import { ConsoleLog } from './ConsoleLog';
import { ToolDeck } from './ToolDeck';
import { DebugModeWizard, WizardConnection } from './debug-mode/Wizard';
import { LeaveDebugMode } from './debug-mode/LeaveDebugMode';
import { debugModeOperations } from './debug-mode/operations';
import { createOwnerCountdown, type Countdown } from './owner-countdown';
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
  const [countdown, setCountdown] = createSignal<Countdown>();
  const owner = createOwnerCountdown(setCountdown);
  onCleanup(
    untrack(() => props.model.onOwnerCountdown((_camera, message) => owner.start(message))),
  );
  onCleanup(() => owner.cancel());

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
              <Match when={camera()}>
                {(current) => (
                  <>
                    <CameraCard camera={current()} model={props.model} />
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
