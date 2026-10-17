import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { DevTools } from '../dev-tools/Page';
import { Progress, type CameraBackend } from '../dev-tools/camera';
test('Dev Tools automatically attaches the already selected backend on handoff', async () => {
  const open = vi.fn(async () => ({
    submit: async () => {},
    poll: async () => ({ progress: Progress.Complete, status: 0, output: new Uint8Array() }),
    usable: () => true,
    close: async () => {},
    abandon: () => {},
  }));
  const pick = vi.fn(async () => ({
    name: 'Selected camera',
    interfaces: [{ id: 0, label: 'Live USB session' }],
  }));
  const backend: CameraBackend = {
    initialize: async () => {},
    pick,
    open,
    drain: () => [],
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(
    () => <DevTools backend={backend} supported={true} connectOnMount={true} />,
    host,
  );
  try {
    await vi.waitFor(() => {
      flush();
      expect(
        (host.querySelector('input[aria-label="Console command"]') as HTMLInputElement)?.disabled,
      ).toBe(false);
    });
    expect(pick).toHaveBeenCalledOnce();
    expect(open).toHaveBeenCalledOnce();
    expect(host.textContent).toContain('Selected camera');
  } finally {
    dispose();
    host.remove();
  }
});
