import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { expect, test, vi } from 'vite-plus/test';
import { CameraReads } from './camera-reads';
import { localFixture } from './local-session-fixture';

test('shared camera actions read and download results, show a remote-claim refusal, and recover on release', async () => {
  const f = localFixture();
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <CameraReads tools={f.tools} />, host);
  const button = (name: string) =>
    [...host.querySelectorAll('button')].find((node) => node.textContent?.trim() === name)!;
  const status = () => host.querySelector('[role="status"]')!.textContent;
  const blobs = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:readings');
  const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
  const complete = () =>
    vi.waitFor(() => {
      flush();
      expect(status()).toBe('Read complete');
    });
  try {
    await f.start();
    for (const name of ['Read device identity', 'Dump configuration', 'Read memory']) {
      expect(button(name).disabled).toBe(false);
      button(name).click();
      await complete();
    }
    expect(f.events.filter((event) => event === 'claim')).toHaveLength(3);
    expect(f.events.filter((event) => event === 'release')).toHaveLength(3);
    f.remote(true);
    button('Read memory').click();
    await vi.waitFor(() => {
      flush();
      expect(status()).toContain('in use by a remote controller');
    });
    expect(f.events.filter((event) => event === 'open')).toHaveLength(3);
    f.remote(false);
    button('Read memory').click();
    await complete();
    button('Download diagnostic bundle').click();
    const bundle = JSON.parse(await (blobs.mock.calls[0][0] as Blob).text());
    expect(bundle.readings).toHaveLength(12);
    expect(bundle.readings.at(-1).data).toEqual(Array(256).fill(42));
    expect(click).toHaveBeenCalledOnce();
  } finally {
    dispose();
    host.remove();
    f.tools.dispose();
    vi.restoreAllMocks();
  }
});
