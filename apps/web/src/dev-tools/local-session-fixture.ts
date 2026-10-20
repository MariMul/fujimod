import { flush } from 'solid-js';
import { expect, vi } from 'vite-plus/test';
import { Progress, type CameraBackend, type CameraLink } from './camera';
import { createDevTools } from './controller';

export function localFixture() {
  const events: string[] = [];
  let remote = false;
  let claimed = false;
  let usable = true;
  let reply = Progress.Complete;
  const inspect = vi.fn(async (request: import('./inspection').InspectionRequest) => {
    events.push(request.kind);
    if (request.kind === 'identity') return { data: new Uint8Array([1]), text: request.selector };
    if (request.kind === 'geometry') return { data: new Uint8Array(), capacity: 4, pageBytes: 2 };
    return { data: new Uint8Array(request.length).fill(42) };
  });
  const backend: CameraBackend = {
    initialize: async () => {},
    pick: async () => ({ name: 'Service camera', interfaces: [{ id: 0, label: 'USB' }] }),
    open: async (): Promise<CameraLink> => {
      events.push('open');
      usable = true;
      return {
        inspect,
        submit: async () => {
          events.push('submit');
        },
        poll: async () => {
          tools.stopPolling();
          events.push('poll');
          return { progress: reply, status: 0, output: new TextEncoder().encode('hello') };
        },
        usable: () => usable,
        close: async () => {
          events.push('close');
          usable = false;
        },
        abandon: () => {
          events.push('abandon');
          usable = false;
        },
      };
    },
    exportUsb: async () => ({
      descriptor: {
        id: 'camera',
        vendor_id: 0x4cb,
        product_id: 0xff80,
        manufacturer: null,
        product: null,
        serial: null,
        interfaces: [],
      },
      execute: async () => ({}),
      close: async () => {},
    }),
    drain: () => [],
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  const tools = createDevTools(backend, true);
  async function start() {
    await tools.start();
    await tools.connect();
    await tools.shareUsb();
    flush();
    expect(tools.localAvailable()).toBe(false);
    tools.sharingReady(() => {
      if (remote)
        throw Error('Camera is in use by a remote controller. Wait for it to release the device.');
      expect(claimed).toBe(false);
      claimed = true;
      events.push('claim');
      return {
        release: async () => {
          events.push('release');
          claimed = false;
        },
        abandon: () => {
          events.push('fence');
          claimed = false;
        },
      };
    });
    events.length = 0;
    flush();
  }
  return {
    tools,
    backend,
    events,
    inspect,
    start,
    claimed: () => claimed,
    remote: (value: boolean) => {
      remote = value;
    },
    reply: (value: Progress) => {
      reply = value;
    },
    breakSession: () => {
      usable = false;
    },
  };
}
