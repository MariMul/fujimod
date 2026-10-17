import { Blob as NodeBlob } from 'node:buffer';
import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { BugReport } from './Page';
import { base64url } from './report-link';
import { tar, link, hash } from './fixtures.test-helper';
import type { ReportConnection } from './usb';
vi.mock('../ui/report-delivery', () => ({ deliverReport: vi.fn() }));
vi.mock('../dev-tools/Page', () => ({
  DevTools: (props: { connectOnMount: boolean }) => (
    <p>Dev Tools {props.connectOnMount ? 'connected' : ''}</p>
  ),
}));
import { deliverReport } from '../ui/report-delivery';
function mount(props: Parameters<typeof BugReport>[0]) {
  vi.stubGlobal('Blob', NodeBlob);
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <BugReport {...props} />, host);
  return {
    host,
    dispose: () => {
      dispose();
      host.remove();
      vi.unstubAllGlobals();
    },
  };
}
test('inline report is ready locally, with no connection or automatic delivery', async () => {
  vi.mocked(deliverReport).mockClear();
  const choose = vi.fn();
  const { host, dispose } = mount({
    hash: hash({ ...link, archive: base64url(tar) }),
    supported: false,
    connection: { choose, dispose: vi.fn() } as unknown as ReportConnection,
  });
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Report ready');
      expect((host.querySelector('button[type=submit]') as HTMLButtonElement).disabled).toBe(false);
    });
    expect(host.textContent).toContain('Nothing is uploaded automatically.');
    expect(choose).not.toHaveBeenCalled();
    expect(deliverReport).not.toHaveBeenCalled();
    const textarea = host.querySelector('textarea')!;
    textarea.value = 'Frozen menu';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    flush();
    host
      .querySelector('form')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    flush();
    expect(deliverReport).toHaveBeenCalledOnce();
    expect(vi.mocked(deliverReport).mock.calls[0][0].issueURL).toContain('Frozen+menu');
  } finally {
    dispose();
  }
});
test('USB acquisition shows progress, verifies readiness and hands off the same connected device', async () => {
  const backend = {};
  const read = vi.fn(async (_id: number, _link: unknown, progress: (n: number) => void) => {
    progress(tar.length);
    return new Blob([tar]);
  });
  const devTools = vi.fn(() => backend),
    connection = {
      initialize: async () => {},
      choose: async () => ({ name: 'Camera', interfaces: [{ id: 0, label: 'USB' }] }),
      read,
      devTools,
      dispose: vi.fn(),
    } as unknown as ReportConnection;
  const { host, dispose } = mount({ hash: hash(), supported: true, connection });
  try {
    await vi.waitFor(() => {
      flush();
      expect((host.querySelector('button') as HTMLButtonElement).disabled).toBe(false);
    });
    host.querySelector('button')!.click();
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Report retrieved from camera.');
    });
    expect(read.mock.calls[0][1]).toEqual(link);
    [...host.querySelectorAll('button')]
      .find((node) => node.textContent === 'Open Dev Tools')!
      .click();
    flush();
    expect(host.textContent).toContain('Dev Tools connected');
    expect(devTools).toHaveBeenCalledOnce();
  } finally {
    dispose();
  }
});
