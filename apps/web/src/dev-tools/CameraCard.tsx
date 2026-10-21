import { For, Show, createSignal } from 'solid-js';
import type { Camera, CameraModel } from './camera-model';

export enum Tone {
  Ready = 'ready',
  Busy = 'busy',
  Problem = 'problem',
  Gone = 'gone',
}

function since(time: number) {
  const minutes = Math.floor((Date.now() - time) / 60000);
  return minutes < 1 ? 'just now' : `${minutes} min ago`;
}

/** One wording for a camera's state, shared by the strip and the card. */
export function cameraState(camera: Camera): { label: string; tone: Tone } {
  if (!camera.present)
    return {
      label: camera.goneAt === undefined ? 'Unplugged' : `Unplugged ${since(camera.goneAt)}`,
      tone: Tone.Gone,
    };
  if (camera.uncertain) return { label: 'Reconnect to continue', tone: Tone.Problem };
  const session = camera.session;
  if (session?.kind === 'local') return { label: session.tool, tone: Tone.Busy };
  if (session?.kind === 'remote')
    return {
      label: camera.remoteActivity?.title ?? `In use by ${session.reason || session.controller}`,
      tone: Tone.Busy,
    };
  return { label: 'Idle', tone: Tone.Ready };
}

export function CameraStrip(props: {
  cameras: readonly Camera[];
  selected?: string;
  connecting: boolean;
  onSelect: (camera: string) => void;
  onConnect: () => void;
}) {
  return (
    <nav class="wb-cams" aria-label="Cameras">
      <For each={props.cameras}>
        {(camera) => {
          const state = () => cameraState(camera);
          return (
            <button
              class={{
                'wb-cam': true,
                selected: camera.id === props.selected,
                gone: !camera.present,
              }}
              aria-current={camera.id === props.selected ? 'true' : undefined}
              onClick={() => props.onSelect(camera.id)}
            >
              <span class="wb-cam-name">
                <i class={`wb-dot ${state().tone}`} aria-hidden="true" />
                {camera.name}
                <Show when={camera.sharing}>
                  {(sharing) => <span class="wb-tag">Shared · {sharing().peers.size}</span>}
                </Show>
              </span>
              <span class="wb-cam-meta">
                {camera.transport === 'usb' ? 'USB' : 'Remote'} · {state().label}
              </span>
            </button>
          );
        }}
      </For>
      <button class="wb-add" disabled={props.connecting} onClick={() => props.onConnect()}>
        {props.connecting ? 'Connecting…' : '+ Connect camera'}
      </button>
    </nav>
  );
}

export function CameraCard(props: { camera: Camera; model: CameraModel }) {
  const [copied, setCopied] = createSignal(false);
  const [sharingBusy, setSharingBusy] = createSignal(false);
  const [sharingError, setSharingError] = createSignal('');
  const state = () => cameraState(props.camera);
  const identity = () => props.camera.identity;
  const product = () =>
    [identity()?.manufacturer ?? identity()?.vendor, identity()?.product].filter(Boolean).join(' ');
  const version = () =>
    [
      identity()?.internalVersion && `Internal ${identity()!.internalVersion}`,
      identity()?.framework && `Framework ${identity()!.framework}`,
    ]
      .filter(Boolean)
      .join(' · ');
  async function setSharing(on: boolean) {
    setSharingBusy(true);
    setSharingError('');
    try {
      await props.model.setSharing(props.camera.id, on);
    } catch (error) {
      setSharingError(error instanceof Error ? error.message : String(error));
    } finally {
      setSharingBusy(false);
    }
  }
  async function copyLink() {
    const link = props.camera.sharing?.link;
    if (!link) return;
    await navigator.clipboard.writeText(link);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }
  return (
    <section class="wb-card" aria-labelledby="wb-camera-name">
      <h2 id="wb-camera-name">{props.camera.name}</h2>
      <Show when={identity()}>
        <dl class="wb-kv">
          <Show when={product()}>
            <dt>Product</dt>
            <dd>{product()}</dd>
          </Show>
          <Show when={version()}>
            <dt>Version</dt>
            <dd>{version()}</dd>
          </Show>
          <Show when={identity()?.serial}>
            <dt>Serial</dt>
            <dd class="wb-serial">{identity()!.serial}</dd>
          </Show>
        </dl>
      </Show>
      <div class={`wb-state ${state().tone}`}>
        <i class={`wb-dot ${state().tone}`} aria-hidden="true" />
        {state().label}
      </div>
      <Show when={props.camera.remoteActivity?.progress}>
        {(progress) => (
          <div
            class="wb-progress"
            role="progressbar"
            aria-label={progress().label}
            aria-valuemin={0}
            aria-valuemax={progress().total}
            aria-valuenow={progress().done}
          >
            <i
              style={{
                width: `${((progress().total > 0 ? Math.min(1, progress().done / progress().total) : 0) * 100).toFixed(1)}%`,
              }}
            />
          </div>
        )}
      </Show>
      <Show when={props.camera.problem}>
        {(problem) => (
          <div class="wb-problem" role="alert">
            <span>{problem()}</span>
            <button class="wb-text" onClick={() => props.model.dismissProblem(props.camera.id)}>
              Dismiss
            </button>
          </div>
        )}
      </Show>
      <Show when={props.camera.transport === 'usb' && props.camera.present}>
        <p class="wb-note">
          In service mode, switching the camera off restores factory settings. Remove the battery to
          leave service mode.
        </p>
        <div class="wb-sharing">
          <label class="wb-switch">
            <input
              type="checkbox"
              role="switch"
              checked={Boolean(props.camera.sharing)}
              disabled={sharingBusy()}
              onChange={(event) => void setSharing(event.currentTarget.checked)}
            />
            <span class="wb-toggle" aria-hidden="true" />
            Sharing
          </label>
          <Show when={props.camera.sharing}>
            {(sharing) => (
              <>
                <span class="wb-muted">
                  · {sharing().peers.size}{' '}
                  {sharing().peers.size === 1 ? 'controller' : 'controllers'}
                </span>
                <button class="wb-text wb-link" onClick={() => void copyLink()}>
                  {copied() ? 'Copied' : 'Copy link'}
                </button>
              </>
            )}
          </Show>
        </div>
        <Show when={sharingError()}>
          <p class="wb-status" role="status">
            {sharingError()}
          </p>
        </Show>
      </Show>
    </section>
  );
}
