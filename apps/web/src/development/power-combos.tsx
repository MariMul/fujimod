import { render } from '@solidjs/web';
import { For, Show } from 'solid-js';
import type { PhysicalControls } from 'fujimod';
import { PowerComboKeys } from '@fujimod/emulator-ui/ui/power-combo';
import './power-combos.css';

const bodies = Object.values(
  import.meta.glob<PhysicalControls>('../../../../catalog/bodies/*/controls.json', {
    eager: true,
    import: 'default',
  }),
).sort((a, b) => a.body_id.localeCompare(b.body_id, 'en', { numeric: true }));

if (import.meta.env.DEV) {
  const host = document.getElementById('power-combos');
  if (!host) throw new Error('Missing power combo review root');
  render(
    () => (
      <>
        <h1>Power-on combos</h1>
        <For each={bodies}>
          {(body) => (
            <section class="phone" data-body={body.body_id}>
              <h2>{body.body_id.toUpperCase()}</h2>
              <Show when={body.boot?.sd} fallback={<p>SD sequence not documented</p>}>
                <PowerComboKeys controls={body} modes={['sd', 'firmware_update']} />
              </Show>
            </section>
          )}
        </For>
      </>
    ),
    host,
  );
}
