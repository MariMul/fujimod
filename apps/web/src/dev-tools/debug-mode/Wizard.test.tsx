import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import DevTools from '../Page';
import { Progress, type CameraBackend } from '../camera';

function page() {
  const host = document.createElement('div');
  document.body.append(host);
  const pick = vi.fn(async () => ({ name: 'X70', interfaces: [{ id: 0, label: 'Debug' }] }));
  const backend: CameraBackend = {
    initialize: async () => {},
    pick,
    open: async () => ({
      submit: async () => {},
      poll: async () => ({ progress: Progress.Complete, status: 0, output: new Uint8Array() }),
      usable: () => true,
      close: async () => {},
      abandon: () => {},
    }),
    drain: () => [],
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  const dispose = render(() => <DevTools backend={backend} supported={true} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((node) => node.textContent?.trim() === name);
  return { host, pick, button, dispose };
}

test('Dev Tools opens on the tool deck with debug-only tools locked', async () => {
  const { host, button, dispose } = page();
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Enter debug mode');
    });
    expect(host.querySelector('input[aria-label="Console command"]')).toBeNull();
    const cards = [...host.querySelectorAll('.td-card')];
    const card = (title: string) =>
      cards.find((node) => node.querySelector('h2')?.textContent === title)!;
    expect(card('Leave debug mode').textContent).toContain('Needs a camera in debug mode');
    expect(card('Share camera').textContent).toContain('Needs a camera in debug mode');
    expect(card('Leave debug mode').querySelector('button')).toBeNull();
    expect(button('Show instructions')).toBeTruthy();
  } finally {
    dispose();
  }
});

test('the wizard explains the procedure, then connects only after the service-mode check', async () => {
  const { host, pick, button, dispose } = page();
  try {
    await vi.waitFor(() => {
      flush();
      expect(button('Show instructions')).toBeTruthy();
    });
    button('Show instructions')!.click();
    flush();
    expect(host.textContent).toContain('Restart the camera into service mode.');
    expect(host.textContent).toContain('Remove the battery to restart normally.');
    button('Begin')!.click();
    await vi.waitFor(() => {
      flush();
      expect(host.querySelector('.dm-camera')).toBeTruthy();
    });
    const x70 = [...host.querySelectorAll<HTMLButtonElement>('.dm-camera')].find((node) =>
      node.textContent?.includes('X70'),
    )!;
    expect(x70.disabled).toBe(false);
    x70.click();
    flush();
    expect(host.textContent).toContain('do not use the OFF switch');
    const allow = button('Allow camera access') as HTMLButtonElement;
    expect(allow.disabled).toBe(true);
    const check = host.querySelector<HTMLInputElement>('.dm-check input')!;
    check.click();
    flush();
    expect(allow.disabled).toBe(false);
    expect(pick).not.toHaveBeenCalled();
    allow.click();
    await vi.waitFor(() => {
      flush();
      expect(pick).toHaveBeenCalledTimes(1);
    });
  } finally {
    dispose();
  }
});
