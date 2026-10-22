import { Show, createSignal, onSettled } from 'solid-js';
import { Icon } from './ui/icons';
import { SpeedGauge } from './ui/speed';
import type { EmulatorFeedbackActions } from './Feedback';
import './viewport-controls.css';

/** The short bar under the display: camera, firmware picker, speed, webcam and feedback. */
export function ViewportControls(props: {
  active: boolean;
  title: string;
  speed?: number;
  webcamSupported?: boolean;
  webcamActive?: boolean;
  webcamBusy?: boolean;
  webcamDisabled?: boolean;
  webcamError?: string;
  toggleWebcam?: () => void;
  modSupported: boolean;
  modEnabled: boolean;
  modDisabled: boolean;
  feedback: EmulatorFeedbackActions;
  selectMod: (enabled: boolean) => void;
}) {
  const [picking, setPicking] = createSignal(false);
  let picker: HTMLDivElement | undefined;
  let trigger: HTMLButtonElement | undefined;
  function close() {
    setPicking(false);
    trigger?.focus();
  }
  onSettled(() => {
    const outside = (event: PointerEvent) => {
      if (picker && event.target instanceof Node && !picker.contains(event.target))
        setPicking(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  });
  const choose = (enabled: boolean) => {
    close();
    if (enabled !== props.modEnabled) props.selectMod(enabled);
  };
  return (
    <div
      id="viewer-tools"
      class="viewport-bar"
      data-ready={String(props.feedback.ready)}
      role="toolbar"
      aria-label="Camera"
      // An idle viewer shows the bar only to recover a saved report.
      hidden={!props.active && !props.feedback.saved}
    >
      <Show when={props.active}>
        <span id="camera-title">{props.title}</span>
        <Show when={props.modSupported}>
          <div
            ref={picker}
            id="firmware-mode"
            class="firmware-picker"
            onKeyDown={(event) => {
              if (event.key === 'Escape' && picking()) {
                event.preventDefault();
                event.stopPropagation();
                close();
              }
            }}
          >
            <button
              ref={trigger}
              type="button"
              aria-haspopup="menu"
              aria-expanded={picking() ? 'true' : 'false'}
              aria-label="Firmware"
              disabled={props.modDisabled}
              onClick={() => setPicking(!picking())}
            >
              {props.modEnabled ? 'FujiMod' : 'Unmodded'}
              <Icon name="chevron" size={12} />
            </button>
            <Show when={picking()}>
              <div class="firmware-menu" role="menu" aria-label="Firmware">
                {[
                  { value: true, label: 'FujiMod' },
                  { value: false, label: 'Unmodded' },
                ].map((option) => (
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={props.modEnabled === option.value ? 'true' : 'false'}
                    data-mod-enabled={String(option.value)}
                    onClick={() => choose(option.value)}
                  >
                    <span class="firmware-check">
                      {props.modEnabled === option.value ? '✓' : ''}
                    </span>
                    {option.label}
                  </button>
                ))}
              </div>
            </Show>
          </div>
        </Show>
        <Show when={props.speed !== undefined && Number.isFinite(props.speed)}>
          <SpeedGauge speed={props.speed!} />
        </Show>
      </Show>
      <span class="viewport-bar-spacer" />
      <Show when={props.active && props.webcamSupported}>
        <Show when={props.webcamError}>
          <span class="webcam-error" role="status">
            {props.webcamError}
          </span>
        </Show>
        <button
          id="webcam-toggle"
          type="button"
          aria-label={
            props.webcamActive || props.webcamBusy ? 'Disable webcam input' : 'Enable webcam input'
          }
          title={
            props.webcamActive || props.webcamBusy ? 'Disable webcam input' : 'Enable webcam input'
          }
          aria-pressed={props.webcamActive ? 'true' : 'false'}
          aria-busy={props.webcamBusy ? 'true' : 'false'}
          disabled={props.webcamDisabled}
          onClick={() => props.toggleWebcam?.()}
        >
          Webcam
        </button>
      </Show>
      <Show when={props.active}>
        <button
          type="button"
          id="feedback-open"
          aria-label="Report a bug or idea"
          title="Report a bug or idea"
          disabled={!props.feedback.ready}
          onClick={() => props.feedback.open()}
        >
          <Icon name="bug" size={14} />
          Bug / Idea
        </button>
      </Show>
      <Show when={props.feedback.saved}>
        <button
          type="button"
          id="feedback-recover"
          aria-label="Download saved report"
          title="Download saved report"
          onClick={() => props.feedback.recover()}
        >
          <Icon name="download" size={14} />
          Saved report
        </button>
      </Show>
    </div>
  );
}
