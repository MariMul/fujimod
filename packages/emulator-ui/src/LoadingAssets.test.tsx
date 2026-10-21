import { render } from '@solidjs/web';
import { createSignal, flush } from 'solid-js';
import { expect, test } from 'vite-plus/test';
import { LoadingAssets } from './LoadingAssets';
import type { AssetProgress } from './engine/download-progress';
test('each asset has its own progress and completed files remain visible through setup', () => {
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
    expect(host.querySelectorAll('progress')[1].hasAttribute('value')).toBe(false);
    expect(host.textContent).toContain('1.0 / 4.0 MiB');
    expect(host.textContent).toContain('~6 s remaining');
    expect(host.textContent).toContain('Waiting…');
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
    expect(host.querySelectorAll('progress')).toHaveLength(0);
  } finally {
    dispose();
    host.remove();
  }
});
