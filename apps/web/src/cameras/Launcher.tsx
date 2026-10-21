import { For, Show, createSignal } from 'solid-js';
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
/** Supported bodies listed before Show All; a search field appears past this. */
export const COLLAPSED_CAMERAS = 8;
export function lastUsed(camera: CameraRecord) {
  return camera.savedAt ?? camera.createdAt;
}
export function usedLabel(time: number, now: number) {
  const seconds = Math.round((now - time) / 1000);
  for (const [unit, size] of units)
    if (seconds >= size) return `Used ${relative.format(-Math.floor(seconds / size), unit)}`;
  return 'Used just now';
}
const searchKey = (text: string) => text.toLowerCase().replace(/[^a-z0-9]/g, '');

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
  const [query, setQuery] = createSignal('');
  const [expanded, setExpanded] = createSignal(false);
  const emulated = () =>
    props.supported
      .filter((camera) => camera.emu)
      .sort((a, b) => a.firmware.model.localeCompare(b.firmware.model, 'en', { numeric: true }));
  const matches = () =>
    emulated().filter((camera) => searchKey(camera.firmware.model).includes(searchKey(query())));
  const shown = () => (query() || expanded() ? matches() : matches().slice(0, COLLAPSED_CAMERAS));
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
        <Show when={props.cameras.length > 0}>
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
        </Show>
        <h3 class={{ 'launcher-add': props.cameras.length > 0 }}>
          {props.cameras.length > 0 ? 'Add a camera' : 'Run your camera in the browser'}
        </h3>
        <ol class="launcher-steps">
          <li>Download your camera’s firmware update from Fujifilm.</li>
          <li>
            <span class="launcher-drop">Drop the downloaded .DAT file here.</span>
            <span class="launcher-touch">Choose the downloaded .DAT file.</span>
          </li>
        </ol>
        <div class="launcher-actions">
          <label class="launcher-button" id="firmware-prompt" for="firmware-file">
            Choose File…
          </label>
          <span class="launcher-note">Firmware stays in this browser.</span>
        </div>
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
        <Show when={emulated().length > COLLAPSED_CAMERAS}>
          <label class="launcher-search">
            <Icon name="search" size={14} />
            <input
              type="search"
              placeholder="Search"
              aria-label="Search supported cameras"
              value={query()}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
        </Show>
        <ul class="launcher-list launcher-table">
          <For each={shown()}>
            {(camera) => (
              <li class="launcher-camera">
                <span class="launcher-camera-name">{camera.firmware.model}</span>
                <span class="launcher-camera-mod">
                  <Show when={camera.mod}>
                    <span class="launcher-tag">FujiMod</span>
                  </Show>
                </span>
                <span class="launcher-version">{camera.firmware.version}</span>
                <a
                  class="launcher-download"
                  href={camera.firmware.download_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label={`Download ${camera.firmware.model} firmware ${camera.firmware.version} from Fujifilm`}
                >
                  Download
                  <Icon name="external" size={11} />
                </a>
              </li>
            )}
          </For>
        </ul>
        <Show when={query() && matches().length === 0}>
          <p class="launcher-empty">No supported camera matches “{query()}”.</p>
        </Show>
        <Show when={!query() && emulated().length > COLLAPSED_CAMERAS}>
          <button class="launcher-more" onClick={() => setExpanded(!expanded())}>
            {expanded() ? 'Show Fewer' : `Show All ${emulated().length}`}
          </button>
        </Show>
      </section>
    </div>
  );
}
