import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { beforeAll, afterEach, expect, test, vi } from 'vite-plus/test';
import { ControlSession, Controls } from './controls';
import type { Engine } from '../engine/api';
import type { Input } from 'fujimod/emulator';
import type { DevicePanel } from './layout';
let dispose: (() => void) | undefined;
beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    disconnect() {}
    unobserve() {}
  };
});
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});
function mount(inputs: Input[], data: DevicePanel) {
  const host = document.createElement('div');
  document.body.append(host);
  const engine = {
    rotate: vi.fn(async () => {}),
    setPosition: vi.fn(async () => {}),
    power: vi.fn(async () => {}),
    press: vi.fn(async () => {}),
    release: vi.fn(async () => {}),
  };
  const viewer = {
    pressFeedback: vi.fn(),
  };
  const session = new ControlSession(
    viewer,
    (error) => {
      throw error;
    },
    async () => {},
  );
  session.render(inputs, engine as unknown as Engine, data);
  dispose = render(() => <Controls session={session} />, host);
  flush();
  return { host, engine, viewer, session };
}
test('every segment selects through the engine and remains synchronized after power off and rollback', async () => {
  const { host, engine, session } = mount(
    [
      {
        id: 'power',
        label: 'Power',
        kind: 'switch',
        supported: true,
        positions: [
          { id: 'OFF', label: 'OFF' },
          { id: 'ON', label: 'ON' },
        ],
        initial: 'ON',
      },
      {
        id: 'focus_mode',
        label: 'Focus',
        kind: 'switch',
        supported: true,
        positions: [
          { id: 'manual', label: 'M' },
          { id: 'single', label: 'S' },
          { id: 'continuous', label: 'C' },
        ],
      },
      { id: 'q', label: 'Q', kind: 'button', supported: true },
    ],
    {
      layout: [
        { kind: 'control', id: 'power', operation: 'power', variant: 'segment' },
        { kind: 'control', id: 'focus_mode', variant: 'segment' },
        { kind: 'control', id: 'q', variant: 'key' },
      ],
      combos: [],
    },
  );
  for (const value of ['single', 'continuous', 'manual']) {
    (
      host.querySelector(
        `[data-id="focus_mode"] button[data-value="${value}"]`,
      ) as HTMLButtonElement
    ).click();
    flush();
    expect(engine.setPosition).toHaveBeenLastCalledWith('focus_mode', value);
    expect(host.querySelector(`[data-value="${value}"]`)?.getAttribute('aria-pressed')).toBe(
      'true',
    );
  }
  (host.querySelector('[data-id="power"] [data-value="OFF"]') as HTMLButtonElement).click();
  flush();
  expect((host.querySelector('button[data-id="q"]') as HTMLButtonElement).disabled).toBe(true);
  expect((host.querySelector('[data-id="focus_mode"] button') as HTMLButtonElement).disabled).toBe(
    true,
  );
  (host.querySelector('[data-id="power"] [data-value="ON"]') as HTMLButtonElement).click();
  await vi.waitFor(() => expect(session.ready).toBe(true));
  flush();
  expect(engine.power).toHaveBeenLastCalledWith(true);
  expect((host.querySelector('button[data-id="q"]') as HTMLButtonElement).disabled).toBe(false);
  session.disable();
  flush();
  expect((host.querySelector('button[data-id="q"]') as HTMLButtonElement).disabled).toBe(true);
  session.resume();
  session.synchronize({ focus_mode: 'single' });
  flush();
  expect(host.querySelector('[data-value="single"]')?.getAttribute('aria-pressed')).toBe('true');
});
test('keyboard dials stop at their physical limits and relative dials preserve detent counts', () => {
  const { host, engine, session } = mount(
    [
      {
        id: 'aperture',
        label: 'Aperture',
        kind: 'ring',
        supported: true,
        positions: [
          { id: '2.8', label: '2.8' },
          { id: '4', label: '4' },
        ],
      },
      { id: 'control_ring', label: 'Command', kind: 'dial', supported: true, relative: true },
    ],
    {
      layout: [
        { kind: 'control', id: 'aperture', variant: 'dial' },
        { kind: 'control', id: 'control_ring', variant: 'ticks' },
      ],
      combos: [],
    },
  );
  const aperture = host.querySelector('[role="slider"][data-id="aperture"]')!;
  aperture.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  flush();
  expect(engine.setPosition).toHaveBeenLastCalledWith('aperture', '4');
  expect(aperture.getAttribute('aria-valuetext')).toBe('4');
  const calls = engine.setPosition.mock.calls.length;
  aperture.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  flush();
  expect(engine.setPosition.mock.calls.length).toBe(calls);
  const command = host.querySelector('[role="slider"][data-id="control_ring"]')!;
  for (const key of ['ArrowRight', 'ArrowRight', 'ArrowLeft'])
    command.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  flush();
  expect(engine.rotate.mock.calls).toEqual([
    ['control_ring', 1],
    ['control_ring', 1],
    ['control_ring', -1],
  ]);
  expect(session.relativeSteps.get('control_ring')).toBe(1);
  expect((command.querySelector('.vals') as HTMLElement).style.backgroundPositionX).toBe('16px');
});
test('a mounted empty panel reacts to the guest input inventory arriving later', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const viewer = { pressFeedback: vi.fn() };
  const session = new ControlSession(
    viewer,
    (error) => {
      throw error;
    },
    async () => {},
  );
  dispose = render(() => <Controls session={session} />, host);
  flush();
  expect(host.querySelectorAll('button').length).toBe(0);
  session.render([{ id: 'q', label: 'Q', kind: 'button', supported: true }], {} as Engine, {
    layout: [{ kind: 'control', id: 'q', variant: 'key' }],
    combos: [],
  });
  flush();
  expect(host.querySelectorAll('button').length).toBe(1);
});
