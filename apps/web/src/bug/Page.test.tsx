import { Blob as NodeBlob } from 'node:buffer';
import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { BugReport } from './Page';
import { base64url } from './report-link';
import { tar, link, hash } from './fixtures.test-helper';
import type { UploadReport } from './upload';
import type { ReportConnection } from './usb';
vi.mock('../dev-tools/Page', () => ({
  DevTools: (props: { connectOnMount: boolean }) => (
    <p>Dev Tools {props.connectOnMount ? 'connected' : ''}</p>
  ),
}));
const receipt = {
  id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  issueURL: 'https://github.com/justjake/fujimod/issues/123',
};
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
function button(host: HTMLElement, name: string) {
  return [...host.querySelectorAll('button')].find((n) => n.textContent === name)!;
}
function submit(host: HTMLElement) {
  host
    .querySelector('form')!
    .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
}
test('two acquisition paths remain available; upload is disabled until an archive is read', async () => {
  const upload = vi.fn();
  const agent = vi
    .spyOn(navigator, 'userAgent', 'get')
    .mockReturnValue('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)');
  const { host, dispose } = mount({ hash: hash(), supported: false, upload });
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Choose from SD card');
    });
    expect(host.textContent).toContain('/FUJIMOD/REPORTS/R1234567.TAR');
    const usb = host.querySelector('[aria-label="USB connection"]')!;
    expect(usb.classList.contains('bug-usb-unavailable')).toBe(true);
    expect(usb.firstElementChild?.textContent).toBe('WebUSB unavailable');
    expect(host.querySelector('.bug-platform')?.textContent).toContain('iPhone · Files');
    expect(host.querySelector('h1')).toBeNull();
    expect(host.textContent).not.toContain('Nothing is uploaded automatically.');
    expect(host.textContent).toContain(
      'Your camera data is private and remains on your device until you press upload.',
    );
    expect(host.textContent).toContain('Chrome or Edge');
    expect(host.textContent).toContain('ready to upload your error report over USB');
    expect(button(host, 'Upload this information').disabled).toBe(true);
    expect((host.querySelector('input[type=checkbox]') as HTMLInputElement).disabled).toBe(true);
    submit(host);
    expect(upload).not.toHaveBeenCalled();
  } finally {
    dispose();
    agent.mockRestore();
  }
});
test('inline report lists real entries locally and uploads only on explicit action', async () => {
  const choose = vi.fn(),
    upload = vi.fn<UploadReport>(async () => receipt),
    remove = vi.fn();
  const { host, dispose } = mount({
    hash: hash({ ...link, archive: base64url(tar) }),
    supported: false,
    upload,
    connection: { choose, remove, dispose: vi.fn() } as unknown as ReportConnection,
  });
  try {
    await vi.waitFor(() => {
      flush();
      expect(button(host, 'Upload this information').disabled).toBe(false);
    });
    expect(host.querySelectorAll('.bug-inventory li').length).toBeGreaterThan(0);
    expect(choose).not.toHaveBeenCalled();
    expect(upload).not.toHaveBeenCalled();
    const textarea = host.querySelector('textarea')!;
    textarea.value = 'Frozen menu';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
    flush();
    submit(host);
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Report saved:');
    });
    expect(upload.mock.calls[0]?.[2]).toBe('Frozen menu');
    expect(remove).not.toHaveBeenCalled();
  } finally {
    dispose();
  }
});
test('failed upload retains USB report; retry reuses ID and deletes only after durable acknowledgement', async () => {
  const remove = vi.fn(async () => {}),
    devTools = vi.fn(() => ({}));
  const upload = vi.fn().mockRejectedValueOnce(Error('Offline')).mockResolvedValue(receipt);
  const connection = {
    initialize: async () => {},
    choose: async () => ({ name: 'Camera', interfaces: [{ id: 0, label: 'USB' }] }),
    read: async () => new Blob([tar]),
    remove,
    devTools,
    dispose: vi.fn(),
  } as unknown as ReportConnection;
  const { host, dispose } = mount({ hash: hash(), supported: true, connection, upload });
  try {
    await vi.waitFor(() => {
      flush();
      expect(button(host, 'Connect camera').disabled).toBe(false);
    });
    button(host, 'Connect camera').click();
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Report retrieved from camera.');
    });
    expect((host.querySelector('input[type=checkbox]') as HTMLInputElement).disabled).toBe(false);
    expect(remove).not.toHaveBeenCalled();
    submit(host);
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Offline');
    });
    expect(remove).not.toHaveBeenCalled();
    submit(host);
    await vi.waitFor(() => {
      flush();
      expect(remove).toHaveBeenCalledOnce();
    });
    expect(upload.mock.calls[0][3]).toBe(upload.mock.calls[1][3]);
    button(host, 'Open Dev Tools').click();
    flush();
    expect(host.textContent).toContain('Dev Tools connected');
  } finally {
    dispose();
  }
});
