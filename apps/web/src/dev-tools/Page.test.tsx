import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { DevTools } from './Page';
import { Progress, type CameraBackend } from './camera';
import { Direction, Layer, Level } from './log';
test('one pane shows console responses and independently filters layers and severity', async () => {
  const host = document.createElement('div');
  document.body.append(host);
  const records = [
    { layer: Layer.Usb, level: Level.Trace, direction: Direction.Out, message: 'bulk bytes' },
    { layer: Layer.App80, level: Level.Debug, direction: Direction.Out, message: 'App80 request' },
  ];
  const backend: CameraBackend = {
    initialize: async () => {},
    pick: async () => ({ name: 'Body C', interfaces: [{ id: 0, label: 'USB' }] }),
    open: async () => ({
      submit: async () => {},
      poll: async () => ({
        progress: Progress.Complete,
        status: 0,
        output: new TextEncoder().encode('camera response'),
      }),
      usable: () => true,
      abandon: () => {},
    }),
    drain: () => records.splice(0),
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  const dispose = render(() => <DevTools backend={backend} supported={true} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((node) => node.textContent === name)!;
  try {
    await vi.waitFor(() => {
      flush();
      expect(button('Connect camera')).toBeTruthy();
    });
    button('Connect camera').click();
    await vi.waitFor(() => {
      flush();
      expect(
        host.querySelector('input[aria-label="Console command"]')!.hasAttribute('disabled'),
      ).toBe(false);
    });
    expect(host.textContent).toContain('App80 request');
    expect(host.textContent).not.toContain('bulk bytes');
    button('USB').click();
    flush();
    expect(host.textContent).toContain('bulk bytes');
    const input = host.querySelector('input[aria-label="Console command"]') as HTMLInputElement;
    input.value = 'help';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flush();
    host
      .querySelector('.dt-composer')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('camera response');
    });
    expect(host.querySelectorAll('[aria-label="Connection log"]')).toHaveLength(1);
    button('Console').click();
    flush();
    expect(host.textContent).not.toContain('camera response');
    button('Console').click();
    flush();
    expect(host.textContent).toContain('camera response');
    const trace = [...host.querySelectorAll('.dt-levels label')]
      .find((node) => node.textContent === 'Trace')!
      .querySelector('input')!;
    trace.click();
    flush();
    expect(host.textContent).not.toContain('bulk bytes');
    expect(host.textContent).toContain('camera response');
  } finally {
    dispose();
    host.remove();
  }
});
test('simulator target is available without physical WebUSB and a pending boot can be cancelled', async () => {
  const host = document.createElement('div');
  document.body.append(host);
  let finish!: (choice: { name: string; interfaces: { id: number; label: string }[] }) => void;
  const pick = vi.fn(
    () =>
      new Promise<{ name: string; interfaces: { id: number; label: string }[] }>((resolve) => {
        finish = resolve;
      }),
  );
  const cancelPending = vi.fn(),
    open = vi.fn();
  const backend: CameraBackend = {
    initialize: async () => {},
    pick,
    open,
    drain: () => [],
    onDisconnect: () => () => {},
    cancelPending,
    dispose: () => {},
  };
  const dispose = render(() => <DevTools backend={backend} supported={false} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((node) => node.textContent === name)!;
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('WebUSB unavailable');
    });
    const target = host.querySelector('[aria-label="Connection target"]') as HTMLSelectElement;
    target.value = 'simulator';
    target.dispatchEvent(new Event('change', { bubbles: true }));
    flush();
    expect(host.querySelector('.dt-status')?.textContent).toBe('Disconnected');
    button('Connect simulator').click();
    flush();
    expect(pick).toHaveBeenCalledWith('simulator');
    button('Cancel').click();
    flush();
    expect(cancelPending).toHaveBeenCalledOnce();
    finish({ name: 'Late emulator', interfaces: [{ id: 0, label: 'USB' }] });
    await Promise.resolve();
    flush();
    expect(open).not.toHaveBeenCalled();
    expect(button('Connect simulator')).toBeTruthy();
  } finally {
    dispose();
    host.remove();
  }
});
