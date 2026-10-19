import { render } from '@solidjs/web';
import { createStore, flush } from 'solid-js';
import { afterEach, expect, test, vi } from 'vite-plus/test';
import { ViewportControls } from './ViewportControls';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});

function mount() {
  const [state, setState] = createStore({
    active: true,
    view: '3d' as '3d' | 'lcd',
    mode: 'inspect' as 'inspect' | 'walk',
    modEnabled: true,
    backend: 'webgl2',
    gpu: true,
    error: '',
    saved: true,
  });
  const face = vi.fn(),
    open = vi.fn(),
    recover = vi.fn();
  const selectMod = vi.fn((enabled: boolean) =>
    setState((s) => {
      s.modEnabled = enabled;
    }),
  );
  const selectRenderer = vi.fn((backend: 'webgl2' | 'webgpu') =>
    setState((s) => {
      s.backend = backend;
    }),
  );
  const toggleMode = vi.fn(() =>
    setState((s) => {
      s.mode = s.mode === 'inspect' ? 'walk' : 'inspect';
    }),
  );
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <ViewportControls
        active={state.active}
        view={state.view}
        hasModel={true}
        title="Camera"
        mode={state.mode}
        error={state.error}
        modSupported={true}
        modEnabled={state.modEnabled}
        modDisabled={false}
        backend={state.backend}
        gpu={state.gpu}
        feedback={{
          ready: true,
          get saved() {
            return state.saved;
          },
          open,
          recover,
        }}
        setView={(view) =>
          setState((s) => {
            s.view = view;
          })
        }
        face={face}
        toggleMode={toggleMode}
        selectMod={selectMod}
        selectRenderer={selectRenderer}
      />
    ),
    host,
  );
  flush();
  const button = (label: string) =>
    [...host.querySelectorAll<HTMLButtonElement>('button')].find(
      (item) => item.textContent === label,
    )!;
  const click = (label: string) => {
    button(label).click();
    flush();
  };
  return {
    host,
    state,
    setState,
    face,
    open,
    recover,
    selectMod,
    selectRenderer,
    toggleMode,
    button,
    click,
  };
}

test('both views, both orientations and both navigation modes remain accessible', () => {
  const t = mount();
  t.click('Screen');
  t.click('Lens');
  expect(t.face.mock.calls).toEqual([['screen'], ['lens']]);
  t.click('Walk');
  expect(t.button('Walk').getAttribute('aria-pressed')).toBe('true');
  t.click('Inspect');
  expect(t.button('Inspect').getAttribute('aria-pressed')).toBe('true');
  expect(t.toggleMode).toHaveBeenCalledTimes(2);
  t.click('Inspect');
  expect(t.toggleMode).toHaveBeenCalledTimes(2);
  t.click('LCD');
  expect(t.button('LCD').getAttribute('aria-pressed')).toBe('true');
  expect(t.host.querySelector('#face-screen')).toBeNull();
  t.click('3D');
  expect(t.button('3D').getAttribute('aria-pressed')).toBe('true');
  expect(t.host.querySelector('#face-screen')).not.toBeNull();
});

test('every firmware and renderer choice dispatches its value and closes the menu', () => {
  const t = mount();
  const details = t.host.querySelector('details')!;
  for (const label of ['Unmodded', 'FujiMod', 'WebGPU', 'WebGL2']) {
    details.open = true;
    t.click(label);
    expect(details.open).toBe(false);
    expect(t.button(label).getAttribute('aria-pressed')).toBe('true');
  }
  expect(t.selectMod.mock.calls).toEqual([[false], [true]]);
  expect(t.selectRenderer.mock.calls).toEqual([['webgpu'], ['webgl2']]);
  t.setState((s) => {
    s.gpu = false;
  });
  flush();
  expect(t.button('WebGPU').disabled).toBe(true);
});

test('feedback, saved reports, escape and outside-click dismissal work from one menu', () => {
  const t = mount(),
    details = t.host.querySelector('details')!,
    summary = t.host.querySelector('summary')!;
  details.open = true;
  t.click('Feedback');
  expect(t.open).toHaveBeenCalledOnce();
  expect(details.open).toBe(false);
  details.open = true;
  t.click('Download saved report');
  expect(t.recover).toHaveBeenCalledOnce();
  expect(details.open).toBe(false);
  details.open = true;
  details.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  expect(details.open).toBe(false);
  expect(document.activeElement).toBe(summary);
  details.open = true;
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(details.open).toBe(false);
  t.setState((s) => {
    s.saved = false;
    s.active = false;
  });
  flush();
  expect(t.host.querySelector('#feedback-recover')).toBeNull();
  expect(t.host.querySelector('#view-3d')).toBeNull();
  expect(t.host.querySelector('#firmware-mode')).toBeNull();
  expect(t.host.querySelector('#feedback-open')).not.toBeNull();
});
