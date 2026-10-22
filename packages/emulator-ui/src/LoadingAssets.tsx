import { For, Show } from 'solid-js';
import type { AssetProgress } from './engine/download-progress';
const mib = (bytes: number) => (bytes / 1048576).toFixed(1);
const size = (bytes: number) =>
  bytes < 1048576 ? `${Math.ceil(bytes / 1024)} KiB` : `${mib(bytes)} MiB`;
export function LoadingAssets(props: { assets: readonly AssetProgress[] }) {
  return (
    <Show when={props.assets.length > 0}>
      <ul class="loading-assets" aria-label="Loading assets">
        <For each={props.assets} keyed={false}>
          {(asset) => (
            <li data-asset={asset().url} data-state={asset().status}>
              <span>{asset().label}</span>
              <span class="asset-size">
                {asset().status === 'waiting'
                  ? 'Waiting…'
                  : asset().status === 'failed'
                    ? 'Failed'
                    : asset().status === 'complete'
                      ? `✓${asset().loaded ? ` ${size(asset().loaded)}` : ''}`
                      : `${mib(asset().loaded)}${asset().total ? ` / ${mib(asset().total!)}` : ''} MiB`}
              </span>
              {/* Every row keeps its bar and rate line so finishing a download does not move the list. */}
              <progress
                max="100"
                value={
                  asset().status === 'complete'
                    ? 100
                    : asset().total
                      ? Math.min(100, (100 * asset().loaded) / asset().total!)
                      : undefined
                }
                aria-label={asset().label}
              />
              <span class="asset-rate">
                {asset().rate && asset().status === 'loading'
                  ? `${mib(asset().rate!)} MiB/s${asset().remaining !== undefined ? ` · ~${asset().remaining} s remaining` : ''}`
                  : '\u00a0'}
              </span>
            </li>
          )}
        </For>
      </ul>
    </Show>
  );
}
