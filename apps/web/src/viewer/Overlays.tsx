import { Show } from 'solid-js';
import type { LCDViewer } from './lcd';
import { InputBadge, inputName, badgeCSS } from '../feedback/input';
export function ViewerOverlays(props: { viewer?: LCDViewer }) {
  return (
    <div
      class="lcd-input-feedback"
      role="status"
      aria-live="polite"
      hidden={!props.viewer?.ui.badge}
      data-input={props.viewer?.ui.badge}
      aria-label={props.viewer?.ui.badge ? inputName(props.viewer.ui.badge) : undefined}
      style={props.viewer?.ui.badge ? badgeCSS : {}}
    >
      <Show when={props.viewer?.ui.badge}>{(id) => <InputBadge id={id()} />}</Show>
    </div>
  );
}
