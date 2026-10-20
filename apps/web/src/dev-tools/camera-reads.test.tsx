import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import DevTools from './Page';
import { Progress, type CameraBackend, type CameraLink } from './camera';
import { createDevTools, LineEnding } from './controller';
import { infoSelectors, memoryRange, type InspectorRead } from './inspection';

function fixture() {
  const inspect = vi.fn<InspectorRead>(async (request) => {
    if (request.kind === 'identity') return { text: request.selector, data: new Uint8Array([1]) };
    if (request.kind === 'geometry')
      return { capacity: 10, pageBytes: 4, data: new Uint8Array([10, 4]) };
    return { data: new Uint8Array(request.length).fill(42) };
  });
  const submit = vi.fn(async () => {});
  const link: CameraLink = {
    inspect,
    submit,
    poll: async () => ({ progress: Progress.Complete, status: 0, output: new Uint8Array() }),
    usable: () => true,
    close: async () => {},
    abandon: () => {},
  };
  const backend: CameraBackend = {
    initialize: async () => {},
    // No firmware/model membership or debug-state hint: a usable ff80 session suffices.
    pick: async () => ({ name: 'Service body', interfaces: [{ id: 0, label: 'ff80' }] }),
    open: async () => link,
    drain: () => [],
    onDisconnect: () => () => {},
    dispose: () => {},
  };
  return { inspect, submit, link, backend };
}

test('camera reads cover all selectors, geometry-bounded configuration and memory into the persistent log and bundle', async () => {
  const f = fixture();
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <DevTools backend={f.backend} supported connectOnMount />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((node) => node.textContent?.trim() === name)!;
  const complete = async () =>
    vi.waitFor(() => {
      flush();
      expect(host.querySelector('.dt-reads [role="status"]')!.textContent).toBe('Read complete');
    });
  const blobUrls = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:diagnostics');
  const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Connected · Service body');
    });
    const log = host.querySelector('.dt-viewport');
    flush();
    button('Read device identity').click();
    await complete();
    expect(f.inspect.mock.calls.map(([request]) => request)).toEqual(
      infoSelectors.map((selector) => ({ kind: 'identity', selector })),
    );
    button('Dump configuration').click();
    await complete();
    expect(f.inspect.mock.calls.slice(7).map(([request]) => request)).toEqual([
      { kind: 'geometry' },
      { kind: 'configuration', offset: 0, length: 4 },
      { kind: 'configuration', offset: 4, length: 4 },
      { kind: 'configuration', offset: 8, length: 2 },
    ]);
    const address = host.querySelector<HTMLInputElement>('.dt-reads input')!;
    address.value = '0x1234';
    address.dispatchEvent(new Event('input', { bubbles: true }));
    flush();
    button('Read memory').click();
    await complete();
    expect(f.inspect).toHaveBeenLastCalledWith({ kind: 'memory', address: 0x1234, length: 256 });
    button('Console').click();
    flush();
    expect(host.querySelector('.dt-viewport')).toBe(log);
    expect(log!.textContent).toContain('Identity · Framework · Framework');
    flush();
    button('Download diagnostic bundle').click();
    const blob = blobUrls.mock.calls[0][0] as Blob;
    const bundle = JSON.parse(await blob.text());
    expect(bundle.schema).toBe('fujimod-inspector-v1');
    expect(bundle.readings).toHaveLength(12);
    expect(bundle.readings.at(-1)).toMatchObject({
      camera: 'Service body',
      data: Array(256).fill(42),
    });
    expect(f.submit).not.toHaveBeenCalled();
    expect(click).toHaveBeenCalledOnce();
  } finally {
    dispose();
    host.remove();
    vi.restoreAllMocks();
  }
});

test('inspection excludes console traffic and rejects mismatched readback lengths', async () => {
  const f = fixture(),
    tools = createDevTools(f.backend, true);
  try {
    await tools.start();
    await tools.connect();
    await tools.readInspection(async (read) => {
      expect(await tools.send('hello', LineEnding.Lf)).toBe(false);
      await expect(tools.readInspection(async () => {})).rejects.toThrow('No idle ff80 session');
      f.inspect.mockResolvedValueOnce({ data: new Uint8Array(1) });
      await expect(read({ kind: 'memory', address: 0, length: 2 })).rejects.toThrow(
        'Read length mismatch',
      );
    });
    expect(f.submit).not.toHaveBeenCalled();
  } finally {
    tools.dispose();
  }
});

test('bounded memory fields reject malformed values and overflowing 32-bit ranges', () => {
  expect(memoryRange('0xfffffff0', '16')).toEqual({ address: 0xfffffff0, length: 16 });
  for (const [address, length] of [
    ['-1', '1'],
    ['0', '0'],
    ['0', '16385'],
    ['0xfffffff0', '17'],
    ['1e3', '1'],
    ['', '2'],
  ])
    expect(() => memoryRange(address, length)).toThrow();
});
