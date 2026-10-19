import { render } from '@solidjs/web';
import { createSignal, flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import { CameraLibrary } from './Library';
import type { CameraRecord } from './store';
let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});
test('each owned camera can be selected, and the active camera can return to the library', () => {
  const records: CameraRecord[] = [1, 2].map((n) => ({
    id: String(n),
    name: `Camera ${n}`,
    firmwareId: 'test',
    firmwareHash: 'a',
    fileName: 'test.dat',
    createdAt: n,
    revision: 0,
    modEnabled: false,
  }));
  const [active, setActive] = createSignal<CameraRecord>();
  const open = vi.fn((camera: CameraRecord) => setActive(camera)),
    close = vi.fn(() => setActive(undefined)),
    upload = vi.fn();
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <CameraLibrary
        cameras={records}
        active={active()}
        busy={false}
        message="Saved"
        error={false}
        open={open}
        close={close}
        upload={upload}
      />
    ),
    host,
  );
  flush();
  for (const record of records) {
    const button = [...host.querySelectorAll<HTMLButtonElement>('.saved-camera')].find((node) =>
      node.textContent?.includes(record.name),
    )!;
    button.click();
    flush();
    expect(open).toHaveBeenLastCalledWith(record);
    expect(host.querySelector('.active-camera')?.textContent).toBe(record.name);
    expect(host.querySelector('.saved-camera')).toBeNull();
    [...host.querySelectorAll('button')].find((node) => node.textContent === 'Cameras')!.click();
    flush();
    expect(close).toHaveBeenCalled();
    expect(host.querySelectorAll('.saved-camera')).toHaveLength(2);
  }
  [...host.querySelectorAll('button')].find((node) => node.textContent === 'New camera')!.click();
  expect(upload).toHaveBeenCalledOnce();
});
test('busy selection is disabled and save failures stay visible', () => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <CameraLibrary
        cameras={[
          {
            id: 'busy',
            name: 'Camera',
            firmwareId: 'test',
            firmwareHash: 'a',
            fileName: 'test.dat',
            createdAt: 1,
            revision: 0,
            modEnabled: false,
          },
        ]}
        busy={true}
        message="Camera not saved · quota exceeded"
        error={true}
        open={() => {}}
        close={() => {}}
        upload={() => {}}
      />
    ),
    host,
  );
  flush();
  expect(host.querySelector('button')?.disabled).toBe(true);
  expect(host.querySelector('[role=status]')?.textContent).toContain('quota exceeded');
  expect(host.querySelector('[role=status]')?.classList.contains('error')).toBe(true);
});
test('the first camera is created through the drop target, without an empty library toolbar', () => {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <CameraLibrary
        cameras={[]}
        busy={false}
        message=""
        error={false}
        open={() => {}}
        close={() => {}}
        upload={() => {}}
      />
    ),
    host,
  );
  flush();
  expect(host.querySelector('#your-cameras')).toBeNull();
  expect(host.querySelector('button')).toBeNull();
});
