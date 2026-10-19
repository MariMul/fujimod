import { For, Show } from 'solid-js';
import presentation from '../../../../bindings/python/python/fujimod/presentation/controls.json';

type Control = { label: string; text?: string; icon?: string };
type Shape = { lines?: number[][][]; circle?: number[] };
const controls: Record<string, Control> = presentation.controls;
const shapes: Record<string, Shape> = presentation.icons;
export const badgeStyle = presentation.badge;
export function inputName(id: string): string {
  const control = controls[id];
  if (!control) throw new Error(`Missing input presentation: ${id}`);
  return control.label;
}

export function InputIcon(props: { id: string }) {
  const shape = () => {
    const name = controls[props.id]?.icon;
    return name ? shapes[name] : undefined;
  };
  return (
    <Show when={shape()}>
      {(value) => (
        <svg
          viewBox="0 0 24 24"
          width={badgeStyle.icon_size}
          height={badgeStyle.icon_size}
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linejoin="round"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <For each={value().lines ?? []}>
            {(line) => <polyline points={line.map((point) => point.join(',')).join(' ')} />}
          </For>
          <Show when={value().circle}>
            {(circle) => (
              <circle cx={circle()[0]} cy={circle()[1]} r={circle()[2]} fill="currentColor" />
            )}
          </Show>
        </svg>
      )}
    </Show>
  );
}
export function InputBadge(props: { id: string }) {
  return (
    <Show
      when={controls[props.id]?.icon}
      fallback={controls[props.id]?.text ?? inputName(props.id)}
    >
      <InputIcon id={props.id} />
    </Show>
  );
}
export const badgeCSS = {
  border: `${badgeStyle.border_width}px solid ${badgeStyle.border}`,
  'border-radius': `${badgeStyle.radius}px`,
  padding: `${badgeStyle.padding_y}px ${badgeStyle.padding_x}px`,
  background: badgeStyle.background,
  color: badgeStyle.foreground,
  font: `600 ${badgeStyle.font_size}px/${badgeStyle.icon_size}px system-ui`,
  bottom: `${badgeStyle.bottom}px`,
};
