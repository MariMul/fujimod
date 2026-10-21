import { Show, onSettled } from 'solid-js';
import { SegmentedControl } from './ui/SegmentedControl';
import { Icon } from './ui/icons';
import { SpeedGauge } from './ui/speed';
import type { EmulatorFeedbackActions } from './Feedback';
import './viewport-controls.css';

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
  let settings!: HTMLDetailsElement;
  let trigger!: HTMLElement;
  function close() {
    settings.open = false;
    trigger.focus();
  }
  onSettled(() => {
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !settings.contains(event.target)) settings.open = false;
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  });
  return (
    <>
      <Show when={props.active}>
        <div class="viewport-status">
          <span id="camera-title">{props.title}</span>
          <Show when={props.speed !== undefined && Number.isFinite(props.speed)}>
            <SpeedGauge speed={props.speed!} />
          </Show>
        </div>
      </Show>
      <div
        id="viewer-tools"
        class="viewport-tools"
        data-ready={String(props.feedback.ready)}
        role="group"
        aria-label="Viewport controls"
      >
        <Show when={props.active && props.webcamSupported}>
          <button
            id="webcam-toggle"
            class="webcam-toggle"
            type="button"
            aria-label={
              props.webcamActive || props.webcamBusy
                ? 'Disable webcam input'
                : 'Enable webcam input'
            }
            title={
              props.webcamActive || props.webcamBusy
                ? 'Disable webcam input'
                : 'Enable webcam input'
            }
            aria-pressed={props.webcamActive ? 'true' : 'false'}
            aria-busy={props.webcamBusy ? 'true' : 'false'}
            disabled={props.webcamDisabled}
            onClick={() => props.toggleWebcam?.()}
          >
            <Icon name="camera" size={18} />
          </button>
          <Show when={props.webcamError}>
            <span class="webcam-error" role="status">
              {props.webcamError}
            </span>
          </Show>
        </Show>
        <details
          ref={settings}
          class="viewport-settings"
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault();
              event.stopPropagation();
              close();
            }
          }}
        >
          <summary
            ref={trigger}
            id="viewer-settings-toggle"
            aria-label="Viewer settings"
            title="Viewer settings"
          >
            <Icon name="more" size={20} />
          </summary>
          <div class="viewport-settings-panel" role="group" aria-label="Viewer settings">
            <Show when={props.active && props.modSupported}>
              <div class="viewport-setting">
                <span class="viewport-setting-label">Firmware</span>
                <SegmentedControl
                  id="firmware-mode"
                  label="Firmware mode"
                  options={[
                    { value: true, label: 'FujiMod', data: { 'data-mod-enabled': 'true' } },
                    { value: false, label: 'Unmodded', data: { 'data-mod-enabled': 'false' } },
                  ]}
                  value={props.modEnabled}
                  disabled={props.modDisabled}
                  onSelect={(enabled) => {
                    close();
                    props.selectMod(enabled);
                  }}
                />
              </div>
            </Show>
            <div class="viewport-menu-actions">
              <button
                type="button"
                id="feedback-open"
                disabled={!props.feedback.ready}
                onClick={() => {
                  close();
                  props.feedback.open();
                }}
              >
                <Icon name="message" />
                <span>Feedback</span>
              </button>
              <Show when={props.feedback.saved}>
                <button
                  type="button"
                  id="feedback-recover"
                  onClick={() => {
                    close();
                    props.feedback.recover();
                  }}
                >
                  <Icon name="download" />
                  <span>Download saved report</span>
                </button>
              </Show>
            </div>
          </div>
        </details>
      </div>
    </>
  );
}
