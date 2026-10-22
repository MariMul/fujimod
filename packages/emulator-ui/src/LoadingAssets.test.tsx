import { render } from '@solidjs/web';
import { createSignal, flush } from 'solid-js';
import { expect, test } from 'vite-plus/test';
import { LoadingAssets } from './LoadingAssets';
import type { AssetProgress } from './engine/download-progress';
test('every step is one row with the same bar, waiting rows empty and finished rows full', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const [assets, setAssets] = createSignal<AssetProgress[]>([
    {
      url: '/camera.mjs',
      label: 'Runtime loader',
      loaded: 1048576,
      total: 4194304,
      status: 'loading',
      rate: 524288,
      remaining: 6,
    },
    { url: '/camera.wasm', label: 'Camera emulator', loaded: 0, status: 'waiting' },
  ]);
  const dispose = render(() => <LoadingAssets assets={assets()} />, host);
  try {
    flush();
    expect(host.querySelectorAll('li')).toHaveLength(2);
    expect(host.querySelector('progress')?.value).toBe(25);
    // A waiting row has an empty bar and no label; there is no rate or ETA text.
    expect(host.querySelectorAll('progress')[1].value).toBe(0);
    expect(host.querySelectorAll('.asset-size')[1].textContent).toBe('');
    expect(host.textContent).toContain('1.0 / 4.0 MiB');
    expect(host.textContent).not.toContain('remaining');
    setAssets([
      {
        url: '/camera.mjs',
        label: 'Runtime loader',
        loaded: 4194304,
        total: 4194304,
        status: 'complete',
      },
    ]);
    flush();
    expect(host.textContent).toContain('Runtime loader');
    expect(host.textContent).toContain('✓ 4.0 MiB');
    // A finished row keeps a full bar so the list does not reflow.
    expect(host.querySelector('progress')?.value).toBe(100);
  } finally {
    dispose();
    host.remove();
  }
});
