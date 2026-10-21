import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import type { Firmware } from '../catalog';
import type { CameraSupport } from '../ui/cameras';
import { CameraLauncher, usedLabel } from './Launcher';
import type { CameraRecord } from './store';
let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});
const now = Date.UTC(2026, 9, 21, 12);
function support(model: string, version: string, sha256: string, emu = true): CameraSupport {
  return {
    firmware: {
      id: model.toLowerCase(),
      model,
      version,
      sha256,
      bytes: 1,
      download_url: `https://example.com/${model}`,
      release_date: '2020-01-01',
    } as Firmware,
    mod: false,
    emu,
  };
}
const supported = [
  { ...support('X70', '1.10', 'x70'), mod: true },
  support('X-M1', '1.50', 'xm1'),
  support('X-T4', '2.12', 'xt4', false),
];
function camera(id: string, firmwareHash: string, used: number): CameraRecord {
  return {
    id,
    name: id,
    firmwareId: firmwareHash,
    firmwareHash,
    fileName: 'FPUPDATE.DAT',
    createdAt: used - 1000,
    savedAt: used,
    revision: 0,
    modEnabled: false,
  };
}
function mount(
  cameras: CameraRecord[],
  busy = false,
  message = '',
  error = false,
  bodies = supported,
) {
  const resume = vi.fn(),
    create = vi.fn();
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <CameraLauncher
        cameras={cameras}
        supported={bodies}
        busy={busy}
        message={message}
        error={error}
        resume={resume}
        create={create}
        now={now}
      />
    ),
    host,
  );
  flush();
  return { host, resume, create };
}
const text = (nodes: NodeListOf<Element>) => [...nodes].map((node) => node.textContent);

test('first use explains where firmware comes from and lists each emulated body', () => {
  const { host } = mount([]);
  expect(host.querySelector('h3')?.textContent).toBe('Run your camera in the browser');
  expect(host.querySelector('.launcher-steps')?.textContent).toContain('Fujifilm');
  expect(host.querySelector('#firmware-prompt')?.getAttribute('for')).toBe('firmware-file');
  expect(host.querySelector('.resume-camera')).toBeNull();
  expect(text(host.querySelectorAll('.launcher-camera'))).toEqual([
    'X-M11.50Download',
    'X70FujiMod1.10Download',
  ]);
  expect(
    [...host.querySelectorAll<HTMLAnchorElement>('.launcher-download')].map((link) => link.href),
  ).toEqual(['https://example.com/X-M1', 'https://example.com/X70']);
  expect(host.querySelector('.launcher-search')).toBeNull();
});

test('a long camera list collapses and filters by search', () => {
  const bodies = Array.from({ length: 50 }, (_, n) => support(`X-T${n + 1}`, '1.00', `t${n}`));
  const { host } = mount([], false, '', false, bodies);
  const names = () => text(host.querySelectorAll('.launcher-camera-name'));
  expect(names()).toHaveLength(8);
  const more = host.querySelector<HTMLButtonElement>('.launcher-more')!;
  expect(more.textContent).toBe('Show All 50');
  more.click();
  flush();
  expect(names()).toHaveLength(50);
  const search = host.querySelector<HTMLInputElement>('input[type=search]')!;
  search.value = 'xt4';
  search.dispatchEvent(new InputEvent('input', { bubbles: true }));
  flush();
  expect(names()).toEqual(['X-T4', ...Array.from({ length: 10 }, (_, n) => `X-T${40 + n}`)]);
  search.value = 'gfx';
  search.dispatchEvent(new InputEvent('input', { bubbles: true }));
  flush();
  expect(names()).toEqual([]);
  expect(host.querySelector('.launcher-empty')?.textContent).toContain('gfx');
});

test('returning visits resume saved cameras by recent use and start new ones from stored firmware', () => {
  const older = camera('a', 'x70', now - 2 * 86400_000),
    newest = camera('b', 'xm1', now - 3 * 3600_000),
    middle = camera('c', 'x70', now - 86400_000);
  const { host, resume, create } = mount([older, newest, middle]);
  expect(text(host.querySelectorAll('.resume-camera'))).toEqual([
    'Resume X-M1Used 3 hours ago',
    'Resume X70Used yesterday',
    'Resume X70Used 2 days ago',
  ]);
  expect(text(host.querySelectorAll('.new-camera'))).toEqual([
    'New X-M1Firmware 1.50',
    'New X70Firmware 1.10',
  ]);
  host.querySelectorAll<HTMLButtonElement>('.resume-camera')[1]!.click();
  expect(resume).toHaveBeenCalledWith(middle);
  host.querySelectorAll<HTMLButtonElement>('.new-camera')[1]!.click();
  expect(create).toHaveBeenCalledWith(middle);
  expect(host.querySelector('h3')?.textContent).toBe('Add a camera');
  expect(host.querySelector('.launcher-steps')?.textContent).toContain('Fujifilm');
  expect(host.querySelector('#firmware-prompt')?.getAttribute('for')).toBe('firmware-file');
});

test('busy rows are disabled and storage failures stay visible', () => {
  const { host } = mount(
    [camera('a', 'x70', now)],
    true,
    'Camera not saved · quota exceeded',
    true,
  );
  for (const button of host.querySelectorAll<HTMLButtonElement>('.launcher-row:is(button)'))
    expect(button.disabled).toBe(true);
  const status = host.querySelector('[role=status]')!;
  expect(status.textContent).toContain('quota exceeded');
  expect(status.classList.contains('error')).toBe(true);
});

test('used labels read as relative dates', () => {
  expect(usedLabel(now - 10_000, now)).toBe('Used just now');
  expect(usedLabel(now - 5 * 60_000, now)).toBe('Used 5 minutes ago');
  expect(usedLabel(now - 40 * 86400_000, now)).toBe('Used last month');
});
