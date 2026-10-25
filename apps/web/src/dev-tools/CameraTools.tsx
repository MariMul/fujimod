import { Show, createSignal } from 'solid-js';
import type { Camera, CameraModel, ToolSession } from './camera-model';
import { zipFiles } from '@fujimod/emulator-ui/archive';

/** Readings collected on one camera, for the diagnostic bundle. */
export interface Readings {
  configuration?: {
    capacity: number;
    pageBytes: number;
    data: Uint8Array;
    complete: boolean;
  };
  errors: { tool: string; time: string; message: string }[];
}
export const emptyReadings = (): Readings => ({ errors: [] });

import { Wizard } from './routes';

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

/** Store-only ZIP: manifest.json plus the camera's bytes as binary files. */
export function diagnosticBundle(camera: Camera, readings: Readings): Blob {
  const files: [string, Uint8Array<ArrayBuffer>][] = [];
  let configuration;
  if (readings.configuration) {
    const { capacity, pageBytes, data, complete } = readings.configuration;
    files.push(['configuration.bin', new Uint8Array(data)]);
    configuration = {
      file: 'configuration.bin',
      capacity,
      pageBytes,
      bytes: data.length,
      complete,
    };
  }
  const manifest = {
    schema: 'fujimod-diagnostics-2',
    createdAt: new Date().toISOString(),
    camera: {
      name: camera.name,
      transport: camera.transport,
      identity: camera.identity ?? {},
    },
    configuration,
    errors: readings.errors,
  };
  files.unshift(['manifest.json', new TextEncoder().encode(JSON.stringify(manifest, null, 2))]);
  return zipFiles(files);
}

/** The tools for the selected camera: Read, Console, Write and Export. */
export function CameraTools(props: {
  camera: Camera;
  model: CameraModel;
  readings: Readings;
  onReadings: (update: (readings: Readings) => Readings) => void;
  onWizard: (wizard: Wizard) => void;
}) {
  const [command, setCommand] = createSignal('');
  const [running, setRunning] = createSignal('');
  const [status, setStatus] = createSignal('');
  const available = () =>
    props.camera.present && !props.camera.uncertain && !props.camera.session && !running();
  type Record = (update: (readings: Readings) => Readings) => void;
  /** Runs `operation` in a session held by `tool`. Reads the camera once, when the action starts. */
  async function run(
    tool: string,
    operation: (session: ToolSession, record: Record) => Promise<void>,
  ) {
    if (!available()) return;
    const id = props.camera.id;
    const model = props.model;
    const record: Record = props.onReadings;
    setRunning(tool);
    setStatus('');
    try {
      await model.withSession(id, tool, (session) => operation(session, record));
    } catch (error) {
      const text = message(error);
      setStatus(text);
      record((readings) => ({
        ...readings,
        errors: [...readings.errors, { tool, time: new Date().toISOString(), message: text }],
      }));
    } finally {
      setRunning('');
    }
  }
  function dumpConfiguration() {
    void run('Dump configuration', async (session, record) => {
      const { capacity, pageBytes } = await session.configurationGeometry();
      // Host allocation budget, independent of camera identity. Never infer geometry.
      if (
        !Number.isInteger(capacity) ||
        capacity <= 0 ||
        capacity > 64 * 1024 * 1024 ||
        !Number.isInteger(pageBytes) ||
        pageBytes < 1
      )
        throw Error('Cannot verify configuration geometry within the 64 MiB host budget.');
      const data = new Uint8Array(capacity);
      let offset = 0;
      try {
        for (; offset < capacity; offset += pageBytes) {
          const piece = await session.readConfiguration(
            offset,
            Math.min(pageBytes, capacity - offset),
          );
          data.set(piece, offset);
        }
      } finally {
        const complete = offset >= capacity;
        record((readings) => ({
          ...readings,
          configuration: {
            capacity,
            pageBytes,
            data: complete ? data : data.slice(0, offset),
            complete,
          },
        }));
      }
    });
  }
  function sendCommand(event: SubmitEvent) {
    event.preventDefault();
    const text = command().trim();
    if (!text) return;
    void run('Console command', async (session) => {
      await session.submitConsole(new TextEncoder().encode(`${text}\n`));
      // Output appears in the log as it arrives; stop waiting after a bounded number of polls.
      for (let poll = 0; poll < 600; poll++) if ((await session.pollConsole()).complete) return;
      throw Error('The command is still running; its output continues in the log.');
    });
  }
  function download() {
    const url = URL.createObjectURL(diagnosticBundle(props.camera, props.readings));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `fujimod-diagnostics-${new Date().toISOString().replaceAll(':', '-')}.zip`;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  const hasReadings = () => Boolean(props.readings.configuration || props.readings.errors.length);
  return (
    <div class="wb-tools-body">
      <section class="wb-section" aria-labelledby="wb-read">
        <h3 id="wb-read" class="wb-label">
          Read
        </h3>
        <button class="wb-btn" disabled={!available()} onClick={dumpConfiguration}>
          {running() === 'Dump configuration' ? 'Dumping configuration…' : 'Dump configuration'}
        </button>
        <button class="wb-btn" onClick={() => props.onWizard(Wizard.MapControls)}>
          Capture buttons
        </button>
      </section>

      <section class="wb-section" aria-labelledby="wb-console">
        <h3 id="wb-console" class="wb-label">
          Console
        </h3>
        <form class="wb-tool" onSubmit={sendCommand}>
          <div class="wb-tool-head">Console command</div>
          <div class="wb-tool-body wb-command">
            <label class="wb-in">
              Command
              <input
                value={command()}
                onInput={(event) => setCommand(event.currentTarget.value)}
                spellcheck={false}
                autocomplete="off"
                autocapitalize="off"
                enterkeyhint="send"
              />
            </label>
            <button class="wb-btn" type="submit" disabled={!available() || !command().trim()}>
              Send
            </button>
          </div>
        </form>
      </section>

      <section class="wb-section" aria-labelledby="wb-write">
        <h3 id="wb-write" class="wb-label wb-write">
          Write
        </h3>
        <div class="wb-tool wb-write">
          <div class="wb-tool-head">
            Debug mode<span class="wb-writes">Changes a camera setting</span>
          </div>
          <p class="wb-explain">
            In debug mode, the camera operates normally, except its USB connection can be used to
            inspect and control the camera’s internal software. USB must be plugged in twice for
            regular file transfer.
          </p>
          <div class="wb-debug-state">
            Debug mode:{' '}
            <strong>
              {props.camera.debugMode === 'on'
                ? 'On'
                : props.camera.debugMode === 'off'
                  ? 'Off'
                  : props.camera.debugMode === 'unknown'
                    ? 'Could not be read'
                    : 'Reading…'}
            </strong>
          </div>
          <div class="wb-pair wb-single">
            <button
              class="wb-btn"
              disabled={
                !available() || props.camera.transport !== 'usb' || props.camera.debugMode === 'off'
              }
              onClick={() => props.onWizard(Wizard.Disable)}
            >
              Disable debug mode
            </button>
          </div>
        </div>
      </section>

      <section class="wb-section" aria-labelledby="wb-export">
        <h3 id="wb-export" class="wb-label">
          Export
        </h3>
        <button class="wb-btn" disabled={!hasReadings()} onClick={download}>
          Download diagnostic bundle
        </button>
      </section>

      <Show when={status()}>
        <p class="wb-status" role="status">
          {status()}
        </p>
      </Show>
    </div>
  );
}
