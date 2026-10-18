import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { webcrypto } from 'node:crypto';
import { expect, test, vi } from 'vite-plus/test';
import Page from './Page';
import { previewAdapter } from './preview';
vi.stubGlobal('crypto', webcrypto);
function button(host: HTMLElement, name: string) {
  return [...host.querySelectorAll('button')].find((node) => node.textContent === name)!;
}
async function click(host: HTMLElement, name: string) {
  await vi.waitFor(() => {
    flush();
    expect(button(host, name)).toBeTruthy();
    expect(button(host, name).disabled).toBe(false);
  });
  button(host, name).click();
  flush();
}
test('full wizard requires explicit erase consent and user-observed menu confirmation', async () => {
  const adapter = previewAdapter('exFAT'),
    host = document.createElement('div');
  document.body.append(host);
  const format = vi.spyOn(adapter, 'format'),
    dispose = render(() => <Page adapter={adapter} preview />, host);
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.querySelector('select')).toBeTruthy();
    });
    const select = host.querySelector('select')!;
    select.value = 'x70-110';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    flush();
    await click(host, 'Continue');
    await click(host, 'Connect camera');
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('exFAT');
    });
    expect(button(host, 'Erase card and format FAT32').disabled).toBe(true);
    button(host, 'Erase card and format FAT32').click();
    expect(format).not.toHaveBeenCalled();
    (host.querySelector('input[type=checkbox]') as HTMLInputElement).click();
    flush();
    await click(host, 'Erase card and format FAT32');
    await vi.waitFor(() => {
      flush();
      expect(button(host, 'Install FujiMod')).toBeTruthy();
    });
    await click(host, 'Install FujiMod');
    await vi.waitFor(() => {
      flush();
      expect(host.textContent).toContain('Files verified');
    });
    expect(host.textContent).toContain('ffboot.bin · SHA-256 verified');
    expect(host.textContent).toContain('ffsys.bin · SHA-256 verified');
    expect(host.textContent).toContain('Remove the battery');
    await click(host, 'Battery reinserted');
    expect(host.textContent).toContain('rear command dial');
    await click(host, 'Camera is on');
    expect(host.textContent).toContain('Press MENU');
    expect(host.textContent).not.toContain('FujiMod is ready');
    await click(host, 'I can see the FujiMod menu');
    expect(host.textContent).toContain('FujiMod is ready');
  } finally {
    dispose();
    host.remove();
  }
});

test('the hardware route cannot select a camera or write data without a verified adapter', async () => {
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(() => <Page />, host);
  try {
    await vi.waitFor(() => {
      flush();
      expect(host.querySelector('select')).toBeTruthy();
    });
    const select = host.querySelector('select')!;
    select.value = 'x70-110';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    flush();
    await click(host, 'Continue');
    expect(button(host, 'Connect camera').disabled).toBe(true);
    expect(host.textContent).toContain('USB SD-card installation is not available');
    expect(host.textContent).not.toContain('Preview · no camera connected');
  } finally {
    dispose();
    host.remove();
  }
});
