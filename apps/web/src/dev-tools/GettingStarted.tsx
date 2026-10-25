import { Show, createSignal } from 'solid-js';
import type { JSX } from '@solidjs/web';
import type {
  Camera,
  CameraModel,
  FlashBackup,
  ServicePreparation,
  StateRegion,
  ToolSession,
} from './camera-model';
import { bulkSelection } from './captures/capture';
import { TarArchive } from './captures/tar';
import { backupChoice, rememberBackup } from './backup-unlock';
import type { DebugModeOperations } from './debug-mode/operations';
import { Wizard } from './routes';
import './getting-started.css';

/** What the path produced on one camera during this page's life. */
export interface PathResults {
  /** The first preparation on this camera: the guard and debug values found before. */
  prepared?: ServicePreparation;
  backup?: { archive: Blob; name: string; flash: FlashBackup } | 'skipped';
  state?: { archive: Blob; name: string; regions: StateRegion[] };
  debugSaved?: boolean;
}

type Step = 'todo' | 'running' | 'done' | 'skipped' | 'locked';

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
function stamp() {
  return new Date().toISOString().replaceAll(':', '-');
}
function hex(value: number) {
  return `0x${value.toString(16).padStart(8, '0')}`;
}
function save(archive: Blob, name: string) {
  const url = URL.createObjectURL(archive);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
function mib(bytes: number) {
  return `${(bytes / (1 << 20)).toFixed(bytes < 1 << 20 ? 2 : 0)} MiB`;
}

/** Dev Tools' numbered path: back up flash, dump state, enable debug mode, restart, capture states. */
export function GettingStarted(props: {
  camera: Camera;
  model: CameraModel;
  results: PathResults;
  onResults: (update: (results: PathResults) => PathResults) => void;
  debug?: DebugModeOperations;
  onWizard: (wizard: Wizard) => void;
}) {
  const [running, setRunning] = createSignal<'backup' | 'state' | 'debug'>();
  const [progress, setProgress] = createSignal('');
  const [failure, setFailure] = createSignal('');
  const selection = () => bulkSelection(props.camera.identity);
  const firmwareId = () => selection()?.firmware.id;
  const available = () =>
    props.camera.present &&
    !props.camera.uncertain &&
    !props.camera.session &&
    !running() &&
    Boolean(firmwareId());
  const unlocked = () =>
    props.results.backup !== undefined || backupChoice(props.camera.identity?.serial) !== undefined;
  const restarted = () => props.camera.debugMode === 'on' && !props.camera.serviceMode;

  const identity = () => ({
    name: props.camera.name,
    serial: props.camera.identity?.serial,
    identity: props.camera.identity ?? {},
    firmware: firmwareId(),
  });

  /** Prepare the service session (idempotent) and run `operation` in the same session. */
  async function step<T>(
    kind: 'backup' | 'state',
    tool: string,
    operation: (session: ToolSession, firmware: string) => Promise<T>,
  ) {
    const firmware = firmwareId();
    const record = props.onResults;
    if (!firmware || !available()) return;
    setRunning(kind);
    setFailure('');
    setProgress('Preparing the service session');
    try {
      return await props.model.withSession(props.camera.id, tool, async (session) => {
        const prepared = await session.prepareServiceSession(firmware);
        record((results) => ({
          ...results,
          prepared: results.prepared ?? prepared,
        }));
        return operation(session, firmware);
      });
    } catch (error) {
      setFailure(message(error));
    } finally {
      setRunning(undefined);
      setProgress('');
    }
  }

  async function backUpFlash() {
    const pieces: { offset: number; data: Uint8Array<ArrayBuffer> }[] = [];
    let read = 0;
    const flash = await step('backup', 'Back up flash', (session, firmware) =>
      session.backupFlash(firmware, ({ offset, data }) => {
        pieces.push({ offset, data: new Uint8Array(data) });
        read += data.length;
        setProgress(`Read ${mib(read)}`);
      }),
    );
    if (!flash) return;
    pieces.sort((a, b) => a.offset - b.offset);
    let end = 0;
    for (const piece of pieces) {
      if (piece.offset !== end) {
        setFailure(`The flash backup has a gap at ${hex(end)}.`);
        return;
      }
      end += piece.data.length;
    }
    if (end !== flash.size) {
      setFailure(`The flash backup covers ${mib(end)} of ${mib(flash.size)}.`);
      return;
    }
    const archive = new TarArchive();
    archive.add(
      'manifest.json',
      new TextEncoder().encode(
        JSON.stringify({ schema: 'fujimod-flash-backup-1', camera: identity(), flash }, null, 2),
      ),
    );
    archive.add(
      'flash.bin',
      pieces.map((piece) => piece.data),
    );
    const name = `fujimod-flash-${props.camera.identity?.serial ?? 'camera'}-${stamp()}.tar`;
    props.onResults((results) => ({
      ...results,
      backup: { archive: archive.blob(), name, flash },
    }));
    rememberBackup(props.camera.identity?.serial, 'done');
  }

  function skipBackup() {
    props.onResults((results) => ({
      ...results,
      backup: results.backup ?? 'skipped',
    }));
    rememberBackup(props.camera.identity?.serial, 'skipped');
  }

  async function dumpState() {
    const archive = new TarArchive();
    const regions: StateRegion[] = [];
    const done = await step('state', 'Dump camera state', async (session, firmware) => {
      await session.dumpState(firmware, ({ data, ...region }) => {
        regions.push(region);
        archive.add(`${region.name}.bin`, new Uint8Array(data));
        setProgress(`${region.name} (${mib(region.size)})`);
      });
      return true;
    });
    if (!done) return;
    archive.add(
      'manifest.json',
      new TextEncoder().encode(
        JSON.stringify(
          {
            schema: 'fujimod-camera-state-1',
            camera: identity(),
            prepared: props.results.prepared,
            regions: regions.map((region) => ({
              ...region,
              file: `${region.name}.bin`,
            })),
          },
          null,
          2,
        ),
      ),
    );
    const name = `fujimod-state-${props.camera.identity?.serial ?? 'camera'}-${stamp()}.tar`;
    props.onResults((results) => ({
      ...results,
      state: { archive: archive.blob(), name, regions },
    }));
  }

  async function enableDebugMode() {
    const firmware = firmwareId();
    if (!firmware || !props.debug || !available() || !unlocked()) return;
    setRunning('debug');
    setFailure('');
    try {
      await props.debug.enter(firmware, setProgress);
      props.onResults((results) => ({ ...results, debugSaved: true }));
    } catch (error) {
      setFailure(message(error));
    } finally {
      setRunning(undefined);
      setProgress('');
    }
  }

  const backupStep = (): Step =>
    running() === 'backup'
      ? 'running'
      : props.results.backup === 'skipped'
        ? 'skipped'
        : props.results.backup
          ? 'done'
          : 'todo';
  const debugStep = (): Step =>
    running() === 'debug'
      ? 'running'
      : props.results.debugSaved || restarted()
        ? 'done'
        : unlocked()
          ? 'todo'
          : 'locked';

  return (
    <section class="gs" aria-labelledby="gs-title">
      <h3 id="gs-title" class="wb-label">
        Get started
      </h3>
      <ol class="gs-steps">
        <Item number={1} title="Back up flash" state={backupStep()}>
          <Show
            when={typeof props.results.backup === 'object' && props.results.backup}
            fallback={
              <div class="gs-row">
                <button
                  class="wb-btn wb-primary"
                  disabled={!available()}
                  onClick={() => void backUpFlash()}
                >
                  Back up flash
                </button>
                <Show when={!props.results.backup}>
                  <button class="wb-text" disabled={Boolean(running())} onClick={skipBackup}>
                    Skip
                  </button>
                </Show>
              </div>
            }
          >
            {(backup) => (
              <button class="wb-btn" onClick={() => save(backup().archive, backup().name)}>
                Download flash backup ({mib(backup().flash.size)})
              </button>
            )}
          </Show>
        </Item>
        <Item
          number={2}
          title="Dump camera state"
          state={running() === 'state' ? 'running' : props.results.state ? 'done' : 'todo'}
        >
          <Show
            when={props.results.state}
            fallback={
              <button class="wb-btn" disabled={!available()} onClick={() => void dumpState()}>
                Dump camera state
              </button>
            }
          >
            {(state) => (
              <button class="wb-btn" onClick={() => save(state().archive, state().name)}>
                Download camera state
              </button>
            )}
          </Show>
        </Item>
        <Item number={3} title="Enable debug mode" state={debugStep()} write>
          <Show when={debugStep() !== 'done'}>
            <button
              class="wb-btn"
              disabled={!available() || !unlocked() || !props.debug}
              onClick={() => void enableDebugMode()}
            >
              Enable debug mode
            </button>
          </Show>
        </Item>
        <Item number={4} title="Restart and reconnect" state={restarted() ? 'done' : 'todo'}>
          <Show when={props.results.debugSaved && !restarted()}>
            <p class="gs-note">
              Remove the battery, reinsert it, switch on and connect again. Plug USB in twice for
              file transfer.
            </p>
          </Show>
        </Item>
        <Item number={5} title="States to capture" state="todo">
          <button
            class="wb-btn"
            disabled={!restarted()}
            onClick={() => props.onWizard(Wizard.StateCaptures)}
          >
            Capture states
          </button>
        </Item>
      </ol>
      <Show when={progress()}>
        <p class="wb-status" role="status">
          {progress()}
        </p>
      </Show>
      <Show when={failure()}>
        <p class="wb-status gs-failure" role="alert">
          {failure()}
        </p>
      </Show>
      <Show when={props.camera.present && !firmwareId()}>
        <p class="wb-status">This firmware has no located bulk reader yet.</p>
      </Show>
    </section>
  );
}

function Item(props: {
  number: number;
  title: string;
  state: Step;
  write?: boolean;
  children?: JSX.Element;
}) {
  return (
    <li class="gs-step" data-state={props.state}>
      <span class="gs-number" aria-hidden="true">
        {props.state === 'done' ? '✓' : props.number}
      </span>
      <div class="gs-body">
        <div class="gs-title">
          {props.title}
          <Show when={props.state === 'skipped'}>
            <span class="gs-tag">Skipped</span>
          </Show>
          <Show when={props.write && props.state !== 'done'}>
            <span class="wb-writes">Changes a camera setting</span>
          </Show>
        </div>
        <Show when={props.state !== 'done' || props.children}>{props.children}</Show>
      </div>
    </li>
  );
}
