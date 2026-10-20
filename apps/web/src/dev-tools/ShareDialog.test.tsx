import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import DevTools from './Page';
import { Progress, type CameraBackend } from './camera';
import type { Callbacks } from '../../../../crates/usb-proxy/web/peer.js';
vi.mock('./dev-share', () => ({ startDevShare: () => () => {} }));
const mock = vi.hoisted(() => ({ callbacks: {} as Callbacks, stop: vi.fn() }));
vi.mock('./sharing', async (actual) => ({
  ...(await actual<typeof import('./sharing')>()),
  ShareHost: {
    create: async (_ticket: unknown, _device: unknown, callbacks: Callbacks) => {
      mock.callbacks = callbacks;
      return {
        stop: mock.stop,
        status: () => ({ online: true }),
        cameraLost: () => {},
        cameraReturned: () => {},
      };
    },
  },
}));
async function mount(nativeSheet: boolean) {
  const calls: string[] = [];
  const write = vi.fn(async (_text: string) => {
    calls.push('copy');
  });
  const share = vi.fn(async (_data: ShareData) => {
    calls.push('sheet');
  });
  const properties = ['clipboard', 'share', 'maxTouchPoints'].map(
    (key) => [key, Object.getOwnPropertyDescriptor(navigator, key)] as const,
  );
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: write },
  });
  Object.defineProperty(navigator, 'share', { configurable: true, value: share });
  Object.defineProperty(navigator, 'maxTouchPoints', {
    configurable: true,
    value: nativeSheet ? 5 : 0,
  });
  const show = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'showModal');
  const close = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'close');
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
  const backend: CameraBackend = {
    initialize: async () => {},
    pick: async () => ({ name: 'Simulator', interfaces: [{ id: 0, label: 'USB' }] }),
    open: async () => ({
      submit: async () => {},
      poll: async () => ({ progress: Progress.Complete, status: 0, output: new Uint8Array() }),
      usable: () => true,
      close: async () => {},
      abandon: () => {},
    }),
    exportUsb: async () => {
      calls.push('export');
      return {
        descriptor: {
          id: '1',
          vendor_id: 0x4cb,
          product_id: 0xff80,
          manufacturer: null,
          product: 'Simulator',
          serial: null,
          interfaces: [],
        },
        execute: async () => ({}),
        close: async () => {},
      };
    },
    drain: () => [],
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <DevTools backend={backend} supported={true} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find(
      (b) => (b.getAttribute('aria-label') ?? b.textContent) === name,
    )!;
  await vi.waitFor(() => {
    flush();
    expect(button('Connect camera')).toBeTruthy();
  });
  button('Connect camera').click();
  await vi.waitFor(() => {
    flush();
    expect(button('Share').disabled).toBe(false);
  });
  button('Share').click();
  flush();
  return {
    host,
    button,
    write,
    share,
    calls,
    dispose: () => {
      dispose();
      host.remove();
      for (const [key, old] of properties) {
        if (old) Object.defineProperty(navigator, key, old);
        else Reflect.deleteProperty(navigator, key);
      }
      if (show) Object.defineProperty(HTMLDialogElement.prototype, 'showModal', show);
      if (close) Object.defineProperty(HTMLDialogElement.prototype, 'close', close);
    },
  };
}
test('Share popup warns, copies a capability URL and logs remote controller connections', async () => {
  const f = await mount(false);
  try {
    const warning = f.host.querySelector('.dt-share-warning')!;
    expect(warning.tagName).toBe('STRONG');
    expect(warning.textContent).toBe(
      'Anyone with a share link will be able to control your USB device.',
    );
    f.button('Simulator').click();
    await vi.waitFor(() => {
      flush();
      expect(f.write).toHaveBeenCalledOnce();
    });
    expect(f.write.mock.calls[0][0]).toContain('#?remotecontrol=');
    expect(f.host.textContent).toContain('Share link copied');
    expect(
      f.host.querySelector('input[aria-label="Console command"]')!.hasAttribute('disabled'),
    ).toBe(true);
    const event = {
      controller: 'manual-tool',
      id: 42,
      timeoutMs: 5000,
      operation: { op: 'transfer_out', endpoint: 1, data: Uint8Array.of(1, 2, 3) },
    };
    mock.callbacks.traffic?.({ ...event, phase: 'submitted' });
    mock.callbacks.traffic?.({
      ...event,
      phase: 'completed',
      elapsedMs: 1,
      completion: { transferred: 3, deviceCount: 0, error: 'stall' },
    });
    await vi.waitFor(() => {
      flush();
      expect(f.host.textContent).toContain('Remote manual-tool');
    });
    expect(f.host.textContent).toContain('transferred 3 B');
    expect(f.host.textContent).toContain('1 failed: stall');
    mock.callbacks.connected?.('2', 'python console hello');
    mock.callbacks.disconnected?.('2', 'done');
    flush();
    expect(f.host.textContent).toContain('Remote controller connected: python console hello');
    expect(f.host.textContent).toContain('Remote controller disconnected: done');
    f.button('Share').click();
    flush();
    f.button('Stop sharing').click();
    flush();
    expect(mock.stop).toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});
test('touch devices open the native share sheet during the camera click activation', async () => {
  const f = await mount(true);
  try {
    f.button('Simulator').click();
    expect(f.calls[0]).toBe('sheet');
    await vi.waitFor(() => {
      flush();
      expect(f.host.textContent).toContain('Camera sharing started');
    });
    expect(f.share.mock.calls[0][0].url).toContain('#?remotecontrol=');
    expect(f.write).not.toHaveBeenCalled();
  } finally {
    f.dispose();
  }
});
