import { createEffect, Show } from 'solid-js';
import type { ShareSession } from './share-session';
import './sharing-status.css';

/** Top-bar state while sharing: one accent chip and a Stop sharing button. */
export function SharingChip(props: { session: ShareSession; onOpen: () => void }) {
  const count = () => props.session.controllers().size;
  const offline = () => props.session.status()?.online === false;
  const battery = () => props.session.status()?.battery;
  return (
    <Show when={props.session.sharing()}>
      <button
        class={offline() ? 'pill share-chip offline' : 'pill share-chip'}
        aria-label="Sharing details"
        onClick={() => props.onOpen()}
      >
        <i aria-hidden="true" />
        {offline() ? 'Sharing · camera offline' : 'Sharing'}
        <Show when={count() > 0}>
          <span> · {count()} connected</span>
        </Show>
        <Show when={battery()}>
          <span class="share-battery"> · Battery {battery()}</span>
        </Show>
      </button>
      <button class="pill" onClick={() => props.session.stop()}>
        Stop sharing
      </button>
    </Show>
  );
}

/** Shown only while a remote controller is connected. */
export function RemoteUseStrip(props: { session: ShareSession }) {
  return (
    <Show when={props.session.controllers().size > 0}>
      <div class="remote-use" role="status">
        Your camera is being used remotely. Keep this tab in front and your computer on to stay
        connected.
      </div>
    </Show>
  );
}

/** One alert at a time; it closes itself when the problem is fixed. */
export function ProblemAlert(props: { session: ShareSession }) {
  let dialog!: HTMLDialogElement;
  createEffect(
    () => props.session.problem(),
    (problem) => {
      if (!dialog) return;
      if (problem && !dialog.open) {
        if (typeof dialog.showModal === 'function') dialog.showModal();
        else dialog.setAttribute('open', '');
      }
      if (!problem && dialog.open) {
        if (typeof dialog.close === 'function') dialog.close();
        else dialog.removeAttribute('open');
      }
      if (problem?.urgent) {
        dialog.classList.remove('shake');
        void dialog.offsetWidth; // restart the animation for each new problem
        dialog.classList.add('shake');
      }
    },
  );
  return (
    <dialog
      ref={dialog}
      class="problem-alert"
      aria-label="Camera problem"
      onClose={() => props.session.dismissProblem()}
    >
      <p>{props.session.problem()?.message}</p>
      <div class="problem-actions">
        <Show when={props.session.problem()?.reconnect}>
          <button
            class="pill primary"
            onClick={() => {
              props.session.unlockAudio();
              void props.session.reconnect();
            }}
          >
            Reconnect camera
          </button>
        </Show>
        <button class="pill" onClick={() => dialog.close()}>
          Close
        </button>
      </div>
    </dialog>
  );
}
