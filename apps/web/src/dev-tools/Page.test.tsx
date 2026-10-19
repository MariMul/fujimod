import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import DevTools from './Page';
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
      close: async () => {},
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
test('simulator target is available without physical WebUSB and a pending boot can be canceled', async () => {
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

const hosts = vi.hoisted(() => ({ create: vi.fn() }));
vi.mock('./sharing', async (actual) => ({
  ...(await actual<typeof import('./sharing')>()),
  ShareHost: { create: hosts.create },
}));
class RegistrySocket {
  static all: RegistrySocket[] = [];
  onopen?: () => void;
  onmessage?: (event: { data: string }) => void;
  onerror?: () => void;
  send = vi.fn();
  close = vi.fn();
  constructor(readonly url: URL) {
    RegistrySocket.all.push(this);
  }
}
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  RegistrySocket.all = [];
  hosts.create.mockReset();
});
/** A ShareHost double that keeps its link when the camera leaves. */
function shareHost(stop: () => void) {
  let status: { online: boolean; reason?: string } = { online: true };
  return {
    stop,
    status: () => status,
    cameraLost: vi.fn((reason: string) => {
      status = { online: false, reason };
    }),
    cameraReturned: vi.fn(() => {
      status = { online: true };
    }),
  };
}
async function mount() {
  vi.stubGlobal('WebSocket', RegistrySocket);
  const stop = vi.fn();
  const sharedHost = shareHost(stop);
  hosts.create.mockResolvedValue(sharedHost);
  let disconnect = () => {};
  const exportUsb = vi.fn(async () => ({
    descriptor: {
      id: 'sim',
      vendor_id: 0x4cb,
      product_id: 0xff80,
      manufacturer: null,
      product: 'X70',
      serial: 'SIM-123',
      interfaces: [],
    },
    execute: async () => ({}),
    close: async () => {},
  }));
  const backend: CameraBackend = {
    initialize: async () => {},
    pick: async () => ({ name: 'X70', interfaces: [{ id: 0, label: 'USB' }] }),
    open: async () => ({
      submit: async () => {},
      poll: async () => ({ progress: Progress.Complete, status: 0, output: new Uint8Array() }),
      usable: () => true,
      close: async () => {},
      abandon: () => {},
    }),
    exportUsb,
    drain: () => [],
    onDisconnect: (callback) => {
      disconnect = callback;
      return () => {};
    },
    dispose: () => {},
  };
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <DevTools backend={backend} supported={true} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((b) => b.textContent === name)!;
  await vi.waitFor(() => {
    flush();
    expect(button('Connect camera')).toBeTruthy();
  });
  return {
    host,
    button,
    exportUsb,
    stop,
    sharedHost,
    disconnect: () => disconnect(),
    dispose: () => {
      dispose();
      host.remove();
    },
  };
}
async function registered(index = 0) {
  await vi.waitFor(() => {
    flush();
    expect(RegistrySocket.all).toHaveLength(index + 1);
  });
  const socket = RegistrySocket.all[index];
  socket.onopen!();
  expect(socket.url.pathname).toBe('/usb-proxy/dev-share');
  const frame = JSON.parse(socket.send.mock.calls[0][0]);
  expect(frame.camera).toEqual({ model: 'X70', serial: 'SIM-123' });
  expect(frame.url).toContain('#?remotecontrol=');
  socket.onmessage!({ data: '{"type":"registered"}' });
  flush();
  return socket;
}
test('auto share registers once and keeps its link while the camera is away', async () => {
  const f = await mount();
  try {
    f.button('Connect camera').click();
    const first = await registered();
    expect(f.host.textContent).toContain('Development share registered');
    const callbacks = hosts.create.mock.calls[0][2];
    callbacks.connected('controller', 'python console hello');
    callbacks.disconnected('controller', 'connection lost without goodbye');
    flush();
    expect(f.host.textContent).toContain('Remote controller connected: python console hello');
    expect(f.host.textContent).toContain(
      'Remote controller disconnected: connection lost without goodbye',
    );
    expect(
      f.host.querySelector('input[aria-label="Console command"]')!.hasAttribute('disabled'),
    ).toBe(true);
    expect(f.exportUsb).toHaveBeenCalledOnce();
    f.disconnect();
    flush();
    // The share and its registration stay; the camera is reported offline.
    expect(first.close).not.toHaveBeenCalled();
    expect(f.stop).not.toHaveBeenCalled();
    expect(f.sharedHost.cameraLost).toHaveBeenCalledWith('unplugged');
    const offline = JSON.parse(first.send.mock.calls.at(-1)![0]);
    expect(offline.status).toEqual({ online: false, reason: 'unplugged' });
    expect(f.host.textContent).toContain('Camera disconnected');
    f.button('Reconnect camera').click();
    await vi.waitFor(() => {
      flush();
      expect(f.sharedHost.cameraReturned).toHaveBeenCalledOnce();
    });
    expect(hosts.create).toHaveBeenCalledOnce();
    expect(JSON.parse(first.send.mock.calls.at(-1)![0]).status).toEqual({ online: true });
    expect(RegistrySocket.all).toHaveLength(1);
    f.dispose();
    expect(first.close).toHaveBeenCalledOnce();
  } finally {
    f.dispose();
  }
});
test('unshare withdraws without immediately sharing again; pagehide also withdraws', async () => {
  const f = await mount();
  const original = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'showModal');
  const originalClose = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'close');
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
  try {
    f.button('Connect camera').click();
    const first = await registered();
    f.button('Share').click();
    flush();
    f.button('Stop sharing').click();
    flush();
    expect(first.close).toHaveBeenCalledOnce();
    expect(hosts.create).toHaveBeenCalledOnce();
    f.button('Disconnect').click();
    await vi.waitFor(() => {
      flush();
      expect(f.button('Connect camera')).toBeTruthy();
    });
    f.button('Connect camera').click();
    const second = await registered(1);
    window.dispatchEvent(new Event('pagehide'));
    expect(second.close).toHaveBeenCalledOnce();
  } finally {
    f.dispose();
    if (original) Object.defineProperty(HTMLDialogElement.prototype, 'showModal', original);
    else Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
    if (originalClose) Object.defineProperty(HTMLDialogElement.prototype, 'close', originalClose);
    else Reflect.deleteProperty(HTMLDialogElement.prototype, 'close');
  }
});
test('disconnect during host setup disposes the late host without registering it', async () => {
  const f = await mount();
  let finish!: (host: { stop: () => void }) => void;
  hosts.create.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  try {
    f.button('Connect camera').click();
    await vi.waitFor(() => {
      flush();
      expect(hosts.create).toHaveBeenCalledOnce();
    });
    f.disconnect();
    flush();
    finish(shareHost(f.stop));
    await vi.waitFor(() => {
      flush();
      expect(f.stop).toHaveBeenCalledOnce();
    });
    expect(RegistrySocket.all).toHaveLength(0);
  } finally {
    f.dispose();
  }
});

test('automatic sharing displays remote traffic and collapses bulk request/completion bursts', async () => {
  const f = await mount();
  try {
    f.button('Connect camera').click();
    await registered();
    const callbacks = hosts.create.mock.calls[0][2];
    for (let id = 0; id < 10000; id++) {
      const event = {
        controller: 'native-tool',
        id,
        device: 'camera',
        timeoutMs: 5000,
        operation: { op: 'transfer_in', endpoint: 0x82, length: 64 },
      };
      callbacks.traffic({ ...event, phase: 'submitted' });
      callbacks.traffic({
        ...event,
        phase: 'completed',
        elapsedMs: 1,
        completion: { transferred: 64, data: '', deviceCount: 0 },
      });
    }
    await vi.waitFor(() => {
      flush();
      expect(f.host.textContent).toContain('10000 submitted, 10000 completed');
    });
    expect(f.host.textContent).toContain('Remote native-tool');
    expect(f.host.textContent).toContain('requested 640000 B, transferred 640000 B');
    expect(
      [...f.host.querySelectorAll('.dt-message')].filter((node) =>
        node.textContent?.includes('transfer_in'),
      ),
    ).toHaveLength(1);
  } finally {
    f.dispose();
  }
});
