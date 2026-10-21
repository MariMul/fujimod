import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { ToolDeck } from './ToolDeck';

test('the splash explains service mode and offers the two ways in', () => {
  const host = document.createElement('div');
  const onServiceMode = vi.fn(),
    onConnect = vi.fn();
  const dispose = render(
    () => <ToolDeck usbSupported onServiceMode={onServiceMode} onConnect={onConnect} />,
    host,
  );
  try {
    flush();
    expect(host.querySelector('.td-enter h2')!.textContent).toBe('Enter service mode');
    expect(host.querySelector('.td-enter')!.textContent).toContain('restores factory settings');
    host.querySelector<HTMLButtonElement>('.td-enter button')!.click();
    expect(onServiceMode).toHaveBeenCalledOnce();
    host.querySelector<HTMLButtonElement>('button[aria-label="Connect camera"]')!.click();
    expect(onConnect).toHaveBeenCalledOnce();
    const simulator = host.querySelector<HTMLAnchorElement>('a[aria-label="Open simulator"]')!;
    expect(simulator.getAttribute('href')).toBe('../?scenario=x70-lcd#workspace');
    expect(host.querySelector<HTMLAnchorElement>('.td-footer-link')!.getAttribute('href')).toBe(
      '../bug/',
    );
  } finally {
    dispose();
  }
});

test('without WebUSB the camera tile is locked with its reason', () => {
  const host = document.createElement('div');
  const onConnect = vi.fn();
  const dispose = render(
    () => <ToolDeck usbSupported={false} onServiceMode={() => {}} onConnect={onConnect} />,
    host,
  );
  try {
    flush();
    const tile = host.querySelector<HTMLButtonElement>('button[aria-label="Connect camera"]')!;
    expect(tile.disabled).toBe(true);
    expect(tile.textContent).toContain('This browser can’t connect to USB. Use Chrome or Edge.');
    tile.click();
    expect(onConnect).not.toHaveBeenCalled();
  } finally {
    dispose();
  }
});
