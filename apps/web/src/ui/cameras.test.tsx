import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import type { Firmware } from '../catalog';
import { CameraTable } from './cameras';
let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});
function firmware(maturity: 'early' | 'supported'): Firmware {
  return {
    id: 'new-body-100',
    body: 'newbody',
    model: 'New body',
    processor: 'EXR Processor II',
    version: '1.00',
    sha256: '1'.repeat(64),
    bytes: 42,
    release_date: '',
    download_url: 'https://example.org/firmware',
    emulate: {
      maturity,
      bundle: 'runtime/new-body-100/',
      sceneInput: false,
    },
  };
}
test('a catalog camera absent from the model list appears under its processor and launches with early status', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const entry = firmware('early'),
    emulate = vi.fn();
  dispose = render(
    () => (
      <CameraTable
        columns={[[{ name: 'EXR Processor II', years: '', models: ['Older body'] }]]}
        cameras={[{ firmware: entry, emu: true, mod: false }]}
        emulate={emulate}
      />
    ),
    host,
  );
  flush();
  expect(host.querySelectorAll('.cam')).toHaveLength(1);
  expect(host.querySelector('.chip.emu.early')?.getAttribute('aria-label')).toBe(
    'Early emulation support',
  );
  host.querySelector<HTMLButtonElement>('button')!.click();
  expect(emulate).toHaveBeenCalledWith(entry);
  expect(host.querySelector('.rest')?.textContent).not.toContain('New body');
});
test('a catalog promotion changes the badge without a body-specific rule', () => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <CameraTable
        columns={[]}
        cameras={[{ firmware: firmware('supported'), emu: true, mod: false }]}
        emulate={() => {}}
      />
    ),
    host,
  );
  flush();
  expect(host.querySelector('.chip.emu.early')).toBeNull();
  expect(host.querySelector('.chip.emu')?.getAttribute('aria-label')).toBe('Emulation supported');
});
test('an unlisted camera without processor metadata does not duplicate known models', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const known = { ...firmware('supported'), model: 'Known', processor: undefined };
  const unlisted = { ...firmware('early'), model: 'Unlisted', processor: undefined };
  dispose = render(
    () => (
      <CameraTable
        columns={[[{ name: 'Reference', years: '', models: ['Known'] }]]}
        cameras={[known, unlisted].map((entry) => ({ firmware: entry, emu: true, mod: false }))}
        emulate={() => {}}
      />
    ),
    host,
  );
  flush();
  expect(host.querySelectorAll('.cam')).toHaveLength(2);
  expect(host.querySelectorAll('button[aria-label="Try Known emulator"]')).toHaveLength(1);
});
