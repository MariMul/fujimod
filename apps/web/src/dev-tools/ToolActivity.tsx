import { Show } from 'solid-js';
import type { ShareSession } from './share-session';
import './tool-activity.css';

/** A remote tool's current activity above the log footer, and its progress
 * bar along the bottom of the log. Both come from the tool's own messages. */
export function ToolActivity(props: { session: ShareSession }) {
  const progress = () => props.session.progress();
  const fraction = () => {
    const p = progress();
    return p && p.total > 0 ? Math.min(1, p.done / p.total) : 0;
  };
  return (
    <>
      <Show when={progress()}>
        {(p) => (
          <div
            class="tool-progress"
            role="progressbar"
            aria-label={p().label}
            aria-valuemin={0}
            aria-valuemax={p().total}
            aria-valuenow={p().done}
          >
            <i style={{ width: `${(fraction() * 100).toFixed(1)}%` }} />
          </div>
        )}
      </Show>
      <Show when={props.session.activity() || progress()}>
        <div class="tool-activity" role="status" aria-live="polite">
          <span>{props.session.activity() ?? progress()?.label}</span>
          <Show when={progress()}>
            {(p) => (
              <span class="tool-activity-count">
                {p().label} {Math.round(fraction() * 100)}%
              </span>
            )}
          </Show>
        </div>
      </Show>
    </>
  );
}
