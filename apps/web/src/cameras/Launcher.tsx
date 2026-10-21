import { For, Show } from 'solid-js';
import { Icon } from '@fujimod/emulator-ui/ui/icons';
import type { CameraSupport } from '../ui/cameras';
import type { CameraRecord } from './store';
import './launcher.css';

const relative = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
const units: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 86400],
  ['month', 30 * 86400],
  ['week', 7 * 86400],
  ['day', 86400],
  ['hour', 3600],
  ['minute', 60],
];
export function lastUsed(camera: CameraRecord) {
  return camera.savedAt ?? camera.createdAt;
}
export function usedLabel(time: number, now: number) {
  const seconds = Math.round((now - time) / 1000);
  for (const [unit, size] of units)
    if (seconds >= size) return `Used ${relative.format(-Math.floor(seconds / size), unit)}`;
  return 'Used just now';
}

/** Idle emulator: resume or create a saved camera, or add firmware for a supported body. */
export function CameraLauncher(props: {
  cameras: CameraRecord[];
  supported: CameraSupport[];
  busy: boolean;
  message: string;
  error: boolean;
  resume: (camera: CameraRecord) => void;
  create: (firmware: CameraRecord) => void;
  now?: number;
}) {
  const emulated = () =>
    props.supported
      .filter((camera) => camera.emu)
      .sort((a, b) => a.firmware.model.localeCompare(b.firmware.model, 'en', { numeric: true }));
  const model = (camera: CameraRecord) =>
    props.supported.find((entry) => entry.firmware.sha256 === camera.firmwareHash)?.firmware ?? {
      model: camera.name,
      version: '',
    };
  const recent = () => [...props.cameras].sort((a, b) => lastUsed(b) - lastUsed(a));
  const firmwares = () =>
    recent().filter(
      (camera, index, all) =>
        all.findIndex((other) => other.firmwareHash === camera.firmwareHash) === index,
    );
  return (
    <div class="launcher">
      <div class="launcher-main">
        <Show
          when={props.cameras.length > 0}
          fallback={
            <>
              <h3>Run your camera in the browser</h3>
              <ol class="launcher-steps">
                <li>Download your camera’s firmware update from Fujifilm.</li>
                <li>Drop the downloaded .DAT file here.</li>
              </ol>
              <div class="launcher-actions">
                <label class="launcher-button" id="firmware-prompt" for="firmware-file">
                  Choose File…
                </label>
                <span class="launcher-note">Firmware stays in this browser.</span>
              </div>
            </>
          }
        >
          <div class="launcher-list" role="list" aria-label="Your cameras">
            <For each={recent()}>
              {(camera) => (
                <button
                  class="launcher-row resume-camera"
                  role="listitem"
                  disabled={props.busy}
                  onClick={() => props.resume(camera)}
                >
                  <span class="launcher-row-text">
                    <strong>Resume {model(camera).model}</strong>
                    <span>{usedLabel(lastUsed(camera), props.now ?? Date.now())}</span>
                  </span>
                  <Icon name="forward" size={16} />
                </button>
              )}
            </For>
          </div>
          <div class="launcher-list" role="list" aria-label="New camera">
            <For each={firmwares()}>
              {(camera) => (
                <button
                  class="launcher-row new-camera"
                  role="listitem"
                  disabled={props.busy}
                  onClick={() => props.create(camera)}
                >
                  <span class="launcher-row-text">
                    <strong>New {model(camera).model}</strong>
                    <Show when={model(camera).version}>
                      <span>Firmware {model(camera).version}</span>
                    </Show>
                  </span>
                  <Icon name="plus" size={16} />
                </button>
              )}
            </For>
          </div>
          <p class="launcher-another">
            Drop another camera’s firmware here, or{' '}
            <label id="firmware-prompt" for="firmware-file">
              choose a file
            </label>
            .
          </p>
        </Show>
        <span
          class={{ 'camera-storage-status': true, error: props.error }}
          role="status"
          aria-live="polite"
        >
          {props.message}
        </span>
      </div>
      <section class="launcher-supported" aria-labelledby="supported-cameras-heading">
        <h4 id="supported-cameras-heading">Supported cameras</h4>
        <ul class="launcher-list">
          <For each={emulated()}>
            {(camera) => (
              <li>
                <a
                  class="launcher-row"
                  href={camera.firmware.download_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label={`Download ${camera.firmware.model} firmware ${camera.firmware.version} from Fujifilm`}
                >
                  <span>{camera.firmware.model}</span>
                  <span class="launcher-version">
                    {camera.firmware.version}
                    <Icon name="external" size={12} />
                  </span>
                </a>
              </li>
            )}
          </For>
        </ul>
      </section>
    </div>
  );
}
