import { Show } from 'solid-js';
import type { ModelViewer } from './model';
import { InputIcon, InputBadge, inputName, badgeCSS } from '../feedback/input';
export function ViewerOverlays(props: { viewer?: ModelViewer }) {
  return (
    <>
      <div
        class={['model-input-marker', { pressed: props.viewer?.ui.marker?.pressed }]}
        hidden={!props.viewer?.ui.marker}
        data-input={props.viewer?.ui.marker?.id}
        style={{
          left: `${props.viewer?.ui.marker?.x ?? 0}px`,
          top: `${props.viewer?.ui.marker?.y ?? 0}px`,
        }}
      >
        <Show when={props.viewer?.ui.marker?.id}>
          {(id) => (
            <Show when={['up', 'down', 'left', 'right'].includes(id())}>
              <InputIcon id={id()} />
            </Show>
          )}
        </Show>
      </div>
      <div
        class={['model-input-feedback', { 'renderer-error': !!props.viewer?.ui.error }]}
        role="status"
        aria-live="polite"
        hidden={!props.viewer?.ui.error && !props.viewer?.ui.badge}
        data-input={props.viewer?.ui.badge}
        aria-label={props.viewer?.ui.badge ? inputName(props.viewer.ui.badge) : undefined}
        style={props.viewer?.ui.badge ? badgeCSS : {}}
      >
        <Show
          when={props.viewer?.ui.error}
          fallback={<Show when={props.viewer?.ui.badge}>{(id) => <InputBadge id={id()} />}</Show>}
        >
          {props.viewer?.ui.error}
        </Show>
      </div>
      <div class="simulation-stick" role="group" aria-label="Walk">
        <span style={{ transform: props.viewer?.ui.stick ?? '' }} />
      </div>
      <Show when={props.viewer?.ui.credit}>
        {(credit) => (
          <a class="environment-credit" href={credit().source}>
            {credit().title} · {credit().author} · CC BY 4.0
          </a>
        )}
      </Show>
    </>
  );
}
