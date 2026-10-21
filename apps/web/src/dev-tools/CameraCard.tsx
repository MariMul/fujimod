import { For, Show, createSignal } from 'solid-js';
import type { Camera, CameraModel } from './camera-model';
import { SegmentedControl } from '../ui/SegmentedControl';

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
                {camera.transport === 'usb' ? 'USB' : 'Remote'} ·{' '}
                {camera.serviceMode ? 'Service mode · ' : ''}
                {state().label}
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
  // "FUJIFILM X100V", without repeating a product that already names its maker.
  const product = () => {
    const maker = identity()?.manufacturer ?? identity()?.vendor;
    const name = identity()?.product;
    if (!name) return maker;
    return maker && !name.toUpperCase().startsWith(maker.toUpperCase()) ? `${maker} ${name}` : name;
  };
  async function setSharing(on: boolean) {
    if (on === Boolean(props.camera.sharing)) return;
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
      <header class="wb-card-head">
        <h2 id="wb-camera-name">{props.camera.name}</h2>
        <span class={`wb-pill ${state().tone}`}>
          <i class={`wb-dot ${state().tone}`} aria-hidden="true" />
          <span class="wb-state">{state().label}</span>
        </span>
      </header>
      <Show when={identity()}>
        <dl class="wb-kv">
          <Show when={product()}>
            <dt>Product</dt>
            <dd>{product()}</dd>
          </Show>
          <Show when={identity()?.internalVersion}>
            <dt>Firmware</dt>
            <dd>
              {identity()!.internalVersion}
              <Show when={identity()?.framework}>
                <span class="wb-muted"> · framework {identity()!.framework}</span>
              </Show>
            </dd>
          </Show>
          <Show when={identity()?.serial}>
            <dt>Serial</dt>
            <dd class="wb-serial" title={identity()!.serial}>
              {identity()!.serial}
            </dd>
          </Show>
        </dl>
      </Show>
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
        <div class="wb-card-row">
          <span class="wb-row-label">Sharing</span>
          <SegmentedControl
            label="Sharing"
            options={[
              { value: false, label: 'Off' },
              { value: true, label: 'On' },
            ]}
            value={Boolean(props.camera.sharing)}
            disabled={sharingBusy()}
            onSelect={(on) => void setSharing(on)}
          />
        </div>
        <Show when={props.camera.sharing}>
          {(sharing) => (
            <div class="wb-share">
              <span class="wb-share-link" title={sharing().link}>
                {sharing().link}
              </span>
              <button class="wb-btn wb-copy" onClick={() => void copyLink()}>
                {copied() ? 'Copied' : 'Copy link'}
              </button>
              <Show when={sharing().peers.size}>
                <span class="wb-muted wb-share-peers">
                  {sharing().peers.size} {sharing().peers.size === 1 ? 'controller' : 'controllers'}{' '}
                  connected
                </span>
              </Show>
            </div>
          )}
        </Show>
        <Show when={sharingError()}>
          <p class="wb-status" role="status">
            {sharingError()}
          </p>
        </Show>
      </Show>
    </section>
  );
}
