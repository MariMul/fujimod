import { For, Show } from 'solid-js';
import type { CameraRecord } from './store';
import './library.css';
export function CameraLibrary(props: {
  cameras: CameraRecord[];
  active?: CameraRecord;
  busy: boolean;
  message: string;
  error: boolean;
  open: (camera: CameraRecord) => void;
  close: () => void;
  upload: () => void;
}) {
  return (
    <Show when={props.active || props.cameras.length > 0 || props.message}>
      <section id="your-cameras" class="camera-library" aria-label="Your cameras">
        <Show when={props.active || props.cameras.length > 0}>
          <div class="camera-library-heading">
            <h2>Your cameras</h2>
            <div class="act">
              <Show when={props.active}>
                <button class="pill" disabled={props.busy} onClick={() => props.close()}>
                  Cameras
                </button>
              </Show>
              <button class="pill" disabled={props.busy} onClick={() => props.upload()}>
                New camera
              </button>
            </div>
          </div>
        </Show>
        <Show when={!props.active}>
          <div class="camera-library-list">
            <For each={props.cameras}>
              {(camera) => (
                <button
                  class="saved-camera"
                  disabled={props.busy}
                  onClick={() => props.open(camera)}
                >
                  <strong>{camera.name}</strong>
                  <span>{new Date(camera.createdAt).toLocaleString()}</span>
                  <span>
                    {camera.savedAt ? (camera.poweredOn ? 'ON · Saved' : 'OFF · Saved') : 'New'}
                  </span>
                </button>
              )}
            </For>
          </div>
        </Show>
        <Show when={props.active}>
          {(camera) => <span class="active-camera">{camera().name}</span>}
        </Show>
        <span
          class={{ 'camera-storage-status': true, error: props.error }}
          role="status"
          aria-live="polite"
        >
          {props.message}
        </span>
      </section>
    </Show>
  );
}
