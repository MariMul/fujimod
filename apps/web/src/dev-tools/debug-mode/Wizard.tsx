import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onSettled,
} from 'solid-js';
import { CameraStore, type CameraRecord } from '../../cameras/store';
import { CameraTarget } from '../camera';
import { ConnectionState, type DevToolsController } from '../controller';
import {
  procedureFor,
  procedures,
  serviceModeEntry,
  unsupportedReason,
  type DebugModeProcedure,
} from './procedures';
import './wizard.css';

/** Firmware-selected workflows on the connected USB session. */
export interface DebugModeOperations {
  armResetGuard(firmwareId: string, progress: (line: string) => void): Promise<void>;
  enter(firmwareId: string, progress: (line: string) => void): Promise<void>;
}
enum Step {
  Overview,
  Camera,
  ServiceMode,
  Connect,
  Enter,
  Restart,
}
const rail = [
  [Step.Camera, 'Camera'],
  [Step.ServiceMode, 'Service mode'],
  [Step.Connect, 'Connect'],
  [Step.Enter, 'Debug mode'],
  [Step.Restart, 'Restart'],
] as const;
interface Choice {
  key: string;
  name: string;
  firmwareId: string;
  saved: boolean;
}
async function choices(): Promise<Choice[]> {
  let saved: CameraRecord[] = [];
  try {
    saved = await new CameraStore().list();
  } catch {
    // Without browser storage the built-in list still offers every supported camera.
  }
  const listed: Choice[] = saved.map((record) => ({
    key: record.id,
    name: record.name,
    firmwareId: record.firmwareId,
    saved: true,
  }));
  for (const procedure of procedures)
    if (!listed.some((choice) => choice.firmwareId === procedure.firmwareId))
      listed.push({
        key: procedure.firmwareId,
        name: `${procedure.model} · ${procedure.version}`,
        firmwareId: procedure.firmwareId,
        saved: false,
      });
  return listed.sort(
    (a, b) => Number(!procedureFor(a.firmwareId)) - Number(!procedureFor(b.firmwareId)),
  );
}
export function DebugModeWizard(props: {
  purpose?: 'service' | 'debug';
  tools: DevToolsController;
  operations?: DebugModeOperations;
  onClose: () => void;
}) {
  const [step, setStep] = createSignal(Step.Overview);
  const [procedure, setProcedure] = createSignal<DebugModeProcedure>();
  const [ready, setReady] = createSignal(false);
  const [lines, setLines] = createSignal<string[]>([]);
  const [failure, setFailure] = createSignal('');
  const [running, setRunning] = createSignal(false);
  const [cameras, setCameras] = createSignal<Choice[]>();
  onSettled(() => {
    void choices().then(setCameras);
  });
  const connection = () => props.tools.connection();
  const connected = createMemo(
    () => connection() === ConnectionState.Connected && props.tools.target() === CameraTarget.Usb,
  );
  const [guarded, setGuarded] = createSignal<DebugModeOperations>();
  const guardReady = () =>
    Boolean(props.operations) && guarded() === props.operations && connected();
  async function armGuard(operations: DebugModeOperations, firmwareId: string | undefined) {
    if (!firmwareId) return;
    setRunning(true);
    setFailure('');
    setLines([]);
    try {
      await operations.armResetGuard(firmwareId, (line) =>
        setLines((previous) => [...previous, line]),
      );
      if (props.operations === operations) setGuarded(operations);
    } catch (error) {
      if (props.operations === operations)
        setFailure(error instanceof Error ? error.message : String(error));
    } finally {
      setRunning(false);
    }
  }
  createEffect(
    () =>
      props.purpose !== 'service' && step() === Step.Connect && props.operations
        ? { operations: props.operations, firmwareId: procedure()?.firmwareId }
        : undefined,
    (request) => {
      if (request) void armGuard(request.operations, request.firmwareId);
    },
  );
  function allowAccess() {
    setStep(Step.Connect);
    void props.tools.connect(CameraTarget.Usb);
  }
  async function enter() {
    if (!props.operations || !guardReady() || running()) return;
    setRunning(true);
    setFailure('');
    setLines([]);
    try {
      await props.operations.enter(procedure()!.firmwareId, (line) =>
        setLines((previous) => [...previous, line]),
      );
      setStep(Step.Restart);
    } catch (error) {
      setFailure(error instanceof Error ? error.message : String(error));
    } finally {
      setRunning(false);
    }
  }
  return (
    <section class="dm" aria-labelledby="dm-title">
      <div class="dm-top">
        <button class="dm-back" onClick={() => props.onClose()}>
          ← Tools
        </button>
        <Show when={step() !== Step.Overview}>
          <ol class="dm-rail" aria-label="Steps">
            <For
              each={
                props.purpose === 'service'
                  ? rail.filter(([id]) => id === Step.ServiceMode || id === Step.Connect)
                  : rail
              }
            >
              {([id, label], index) => (
                <li
                  class={{ done: step() > id, current: step() === id }}
                  aria-current={step() === id ? 'step' : undefined}
                >
                  <span class="dm-rail-number">{String(index() + 1).padStart(2, '0')}</span>
                  <span class="dm-rail-label">{label}</span>
                </li>
              )}
            </For>
          </ol>
        </Show>
      </div>
      <Switch>
        <Match when={step() === Step.Overview}>
          <div class="dm-panel">
            <h1 id="dm-title" class="dm-title">
              {props.purpose === 'service' ? 'Enter service mode' : 'Enter debug mode'}
            </h1>
            <Show
              when={props.purpose !== 'service'}
              fallback={
                <p class="dm-lead">Service mode exposes the camera’s ff80 USB connection.</p>
              }
            >
              <p class="dm-lead">
                In debug mode your camera works normally, but its USB connection can also inspect
                and control the camera’s software. Ordinary file transfer needs USB plugged in
                twice.
              </p>
              <ol class="dm-plan">
                <li>
                  <span>01</span>Restart the camera into service mode.
                </li>
                <li>
                  <span>02</span>Connect it here and turn on the hidden Debug Mode setting.
                </li>
                <li>
                  <span>03</span>Remove the battery to restart normally.
                </li>
              </ol>
            </Show>
            <p class="dm-caution">
              Service mode restores factory settings when the camera is switched off. Removing the
              battery avoids that. You need a USB cable and about five minutes.
            </p>
            <button
              class="dm-button dm-next"
              onClick={() => setStep(props.purpose === 'service' ? Step.ServiceMode : Step.Camera)}
            >
              Begin
            </button>
          </div>
        </Match>
        <Match when={step() === Step.Camera}>
          <div class="dm-panel">
            <h1 id="dm-title" class="dm-title">
              Choose your camera
            </h1>
            <Show when={cameras()} fallback={<p class="dm-lead">Loading cameras…</p>}>
              <div class="dm-cameras">
                <For each={cameras()}>
                  {(choice) => {
                    const known = procedureFor(choice.firmwareId);
                    return (
                      <button
                        class="dm-camera"
                        disabled={!known || Boolean(known.unsupported)}
                        onClick={() => {
                          setProcedure(known);
                          setReady(false);
                          setStep(Step.ServiceMode);
                        }}
                      >
                        <strong>{choice.name}</strong>
                        <span>
                          {known && !known.unsupported
                            ? choice.saved
                              ? 'Saved in the emulator'
                              : 'Supported'
                            : `Not supported yet. ${unsupportedReason(choice.firmwareId)}`}
                        </span>
                      </button>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>
        </Match>
        <Match when={step() === Step.ServiceMode}>
          <div class="dm-panel">
            <h1 id="dm-title" class="dm-title">
              Restart into service mode
            </h1>
            <ol class="dm-entry">
              <For
                each={procedure()?.entry ?? (props.purpose === 'service' ? serviceModeEntry : [])}
              >
                {(line) => <li>{line}</li>}
              </For>
            </ol>
            <div class="dm-warning" role="note">
              <strong>In service mode, do not use the OFF switch.</strong>
              Switching off in service mode restores factory settings. To restart, remove the
              battery.
            </div>
            <label class="dm-check">
              <input
                type="checkbox"
                checked={ready()}
                onChange={(event) => setReady(event.currentTarget.checked)}
              />
              <span>I’m in service mode, the green lamp is on, and USB is connected.</span>
            </label>
            <div class="dm-actions">
              <button
                class="dm-button dm-secondary"
                onClick={() => setStep(props.purpose === 'service' ? Step.Overview : Step.Camera)}
              >
                Back
              </button>
              <button
                class={{ 'dm-button': true, 'dm-next': ready() }}
                disabled={!ready()}
                onClick={allowAccess}
              >
                Allow camera access
              </button>
            </div>
          </div>
        </Match>
        <Match when={step() === Step.Connect}>
          <div class="dm-panel">
            <Show when={connection() === ConnectionState.Choosing}>
              <div class="dm-pointer" aria-hidden="true">
                ↑
              </div>
            </Show>
            <h1 id="dm-title" class="dm-title">
              Allow camera access
            </h1>
            <Switch>
              <Match when={connected()}>
                <p class="dm-lead dm-ok">Camera connected.</p>
                <button
                  class="dm-button dm-next"
                  disabled={props.purpose !== 'service' && (!guardReady() || running())}
                  onClick={() =>
                    props.purpose === 'service' ? props.onClose() : setStep(Step.Enter)
                  }
                >
                  {props.purpose === 'service' ? 'Open workbench' : 'Continue'}
                </button>
              </Match>
              <Match
                when={[
                  ConnectionState.Choosing,
                  ConnectionState.Selecting,
                  ConnectionState.Opening,
                ].includes(connection())}
              >
                <p class="dm-lead">
                  In the browser’s dialog at the top, choose the unknown FUJIFILM device, then press
                  Connect.
                </p>
                <p class="dm-caution">
                  No devices listed? The camera isn’t in service mode yet. Cancel, go back and
                  restart it.
                </p>
              </Match>
              <Match when={connection() === ConnectionState.Unsupported}>
                <p class="dm-lead">
                  This browser can’t connect to USB devices. Use Chrome or Edge.
                </p>
              </Match>
              <Match when={true}>
                <p class="dm-lead">The camera isn’t connected.</p>
                <div class="dm-actions">
                  <button class="dm-button dm-secondary" onClick={() => setStep(Step.ServiceMode)}>
                    Back
                  </button>
                  <button class="dm-button dm-next" onClick={allowAccess}>
                    Try again
                  </button>
                </div>
              </Match>
            </Switch>
            <Show when={lines().length}>
              <pre class="dm-log" aria-live="polite">
                <For each={lines()}>{(line) => <span>{`> ${line}\n`}</span>}</For>
              </pre>
            </Show>
            <Show when={failure()}>
              <p class="dm-error" role="alert">
                {failure()}
              </p>
              <button
                class="dm-button"
                disabled={running() || !props.operations}
                onClick={() => {
                  if (props.operations) void armGuard(props.operations, procedure()?.firmwareId);
                }}
              >
                Try again
              </button>
            </Show>
          </div>
        </Match>
        <Match when={step() === Step.Enter}>
          <div class="dm-panel">
            <h1 id="dm-title" class="dm-title">
              Turn on debug mode
            </h1>
            <button
              class={{
                'dm-button': true,
                'dm-huge': true,
                'dm-next': guardReady() && !running(),
              }}
              disabled={!guardReady() || running()}
              onClick={() => void enter()}
            >
              Enter debug mode
            </button>
            <Show when={!connected() && props.operations}>
              <p class="dm-caution">The camera disconnected. Go back and connect it again.</p>
            </Show>
            <Show when={lines().length}>
              <pre class="dm-log" aria-live="polite">
                <For each={lines()}>{(line) => <span>{`> ${line}\n`}</span>}</For>
              </pre>
            </Show>
            <Show when={failure()}>
              <p class="dm-error" role="alert">
                {failure()}
              </p>
            </Show>
          </div>
        </Match>
        <Match when={step() === Step.Restart}>
          <div class="dm-panel">
            <h1 id="dm-title" class="dm-title">
              Restart the camera
            </h1>
            <ol class="dm-entry">
              <li>Remove the battery. Do not use the OFF switch.</li>
              <li>Unplug USB and switch the camera to OFF.</li>
              <li>Put the battery back and switch the camera ON normally.</li>
            </ol>
            <p class="dm-lead">
              Your camera now works as usual. For ordinary file transfer, plug USB in twice.
            </p>
            <button class="dm-button dm-next" onClick={() => props.onClose()}>
              Done
            </button>
          </div>
        </Match>
      </Switch>
    </section>
  );
}
