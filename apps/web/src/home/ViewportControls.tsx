import { For, Show, onSettled } from 'solid-js';
import { Icon } from '../ui/icons';
import { SpeedGauge } from '../ui/speed';
import type { EmulatorFeedbackActions } from './Feedback';
import './viewport-controls.css';

export function ViewportControls(props: {
  active: boolean;
  view: '3d' | 'lcd';
  hasModel: boolean;
  title: string;
  speed?: number;
  mode: 'inspect' | 'walk';
  error?: string;
  modSupported: boolean;
  modEnabled: boolean;
  modDisabled: boolean;
  backend: string;
  gpu: boolean;
  feedback: EmulatorFeedbackActions;
  setView: (view: '3d' | 'lcd') => void;
  face: (face: 'screen' | 'lens') => void;
  toggleMode: () => void;
  selectMod: (enabled: boolean) => void;
  selectRenderer: (backend: 'webgl2' | 'webgpu') => void;
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
        class={['viewport-tools', { 'viewport-tools-idle': !props.active }]}
        data-ready={String(props.feedback.ready)}
        role="group"
        aria-label="Viewport controls"
      >
        <Show when={props.active}>
          <div class="viewport-tool-group" role="group" aria-label="Viewer mode">
            <For each={['3d', 'lcd'] as const}>
              {(view) => (
                <button
                  type="button"
                  id={`view-${view}`}
                  class={{ selected: props.view === view }}
                  aria-pressed={props.view === view ? 'true' : 'false'}
                  hidden={view === '3d' && !props.hasModel}
                  onClick={() => props.setView(view)}
                >
                  {view === '3d' ? '3D' : 'LCD'}
                </button>
              )}
            </For>
          </div>
          <Show when={props.view === '3d' && props.hasModel}>
            <div class="viewport-tool-group" role="group" aria-label="Camera orientation">
              <button
                type="button"
                id="face-screen"
                disabled={!!props.error}
                onClick={() => props.face('screen')}
              >
                <Icon name="monitor" />
                <span>Screen</span>
              </button>
              <button
                type="button"
                id="face-lens"
                disabled={!!props.error}
                onClick={() => props.face('lens')}
              >
                <Icon name="camera" />
                <span>Lens</span>
              </button>
            </div>
            <div class="viewport-tool-group" role="group" aria-label="Navigation mode">
              <For each={['inspect', 'walk'] as const}>
                {(mode) => (
                  <button
                    type="button"
                    class={{ selected: props.mode === mode }}
                    aria-pressed={props.mode === mode ? 'true' : 'false'}
                    disabled={!!props.error}
                    onClick={() => {
                      if (props.mode !== mode) props.toggleMode();
                    }}
                  >
                    <Icon name={mode === 'inspect' ? 'scan' : 'walk'} />
                    <span>{mode === 'inspect' ? 'Inspect' : 'Walk'}</span>
                  </button>
                )}
              </For>
            </div>
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
                <div
                  id="firmware-mode"
                  class="viewport-setting-options"
                  role="group"
                  aria-label="Firmware mode"
                >
                  <For each={[true, false]}>
                    {(enabled) => (
                      <button
                        type="button"
                        class={{ selected: props.modEnabled === enabled }}
                        data-mod-enabled={String(enabled)}
                        aria-pressed={props.modEnabled === enabled ? 'true' : 'false'}
                        disabled={props.modDisabled}
                        onClick={() => {
                          close();
                          props.selectMod(enabled);
                        }}
                      >
                        {enabled ? 'FujiMod' : 'Unmodded'}
                      </button>
                    )}
                  </For>
                </div>
              </div>
            </Show>
            <Show when={props.active && props.view === '3d' && props.hasModel}>
              <div class="viewport-setting">
                <span class="viewport-setting-label">Renderer</span>
                <div class="viewport-setting-options" role="group" aria-label="3D renderer">
                  <For each={['webgl2', 'webgpu'] as const}>
                    {(backend) => (
                      <button
                        type="button"
                        class={{ selected: props.backend === backend }}
                        aria-pressed={props.backend === backend ? 'true' : 'false'}
                        disabled={backend === 'webgpu' && !props.gpu}
                        onClick={() => {
                          close();
                          if (props.backend !== backend) props.selectRenderer(backend);
                        }}
                      >
                        {backend === 'webgl2' ? 'WebGL2' : 'WebGPU'}
                      </button>
                    )}
                  </For>
                </div>
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
