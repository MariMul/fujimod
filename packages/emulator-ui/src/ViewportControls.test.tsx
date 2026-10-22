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
    modEnabled: true,
    saved: true,
    webcamActive: false,
    webcamSupported: true,
    webcamBusy: false,
    webcamDisabled: false,
    webcamError: '',
  });
  const open = vi.fn(),
    recover = vi.fn();
  const toggleWebcam = vi.fn(() =>
    setState((s) => {
      s.webcamActive = !s.webcamActive;
    }),
  );
  const selectMod = vi.fn((enabled: boolean) =>
    setState((s) => {
      s.modEnabled = enabled;
    }),
  );
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <ViewportControls
        active={state.active}
        title="Camera"
        webcamSupported={state.webcamSupported}
        webcamBusy={state.webcamBusy}
        webcamDisabled={state.webcamDisabled}
        webcamError={state.webcamError}
        webcamActive={state.webcamActive}
        toggleWebcam={toggleWebcam}
        modSupported={true}
        modEnabled={state.modEnabled}
        modDisabled={false}
        feedback={{
          ready: true,
          get saved() {
            return state.saved;
          },
          open,
          recover,
        }}
        selectMod={selectMod}
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
    open,
    recover,
    selectMod,
    toggleWebcam,
    button,
    click,
  };
}

test('the visible webcam button toggles capture and reflects its state', () => {
  const t = mount();
  const button = t.host.querySelector<HTMLButtonElement>('#webcam-toggle')!;
  expect(button.getAttribute('aria-label')).toBe('Enable webcam input');
  button.click();
  flush();
  expect(t.toggleWebcam).toHaveBeenCalledOnce();
  expect(button.getAttribute('aria-pressed')).toBe('true');
  expect(button.getAttribute('aria-label')).toBe('Disable webcam input');
  button.click();
  flush();
  expect(button.getAttribute('aria-pressed')).toBe('false');
});

test('every firmware choice dispatches its value and closes the menu', () => {
  const t = mount();
  const details = t.host.querySelector('details')!;
  for (const label of ['Unmodded', 'FujiMod']) {
    details.open = true;
    t.click(label);
    expect(details.open).toBe(false);
    expect(t.button(label).getAttribute('aria-pressed')).toBe('true');
  }
  expect(t.selectMod.mock.calls).toEqual([[false], [true]]);
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
  expect(t.host.querySelector('#view-3d,#view-lcd,#face-screen,#face-lens')).toBeNull();
  expect(t.host.querySelector('[aria-label="3D renderer"]')).toBeNull();
  expect(t.host.querySelector('[aria-label="Navigation mode"]')).toBeNull();
  expect(t.host.querySelector('#firmware-mode')).toBeNull();
  // An idle viewer has no menu unless a saved report needs recovering.
  expect(t.host.querySelector('#viewer-settings-toggle')).toBeNull();
  t.setState((s) => {
    s.saved = true;
  });
  flush();
  expect(t.host.querySelector('#feedback-recover')).not.toBeNull();
});

test('webcam support, pending permission, disabled state and errors follow the selected camera', () => {
  const t = mount();
  t.setState((s) => {
    s.webcamBusy = true;
    s.webcamError = 'Camera permission was denied.';
  });
  flush();
  const button = t.host.querySelector<HTMLButtonElement>('#webcam-toggle')!;
  expect(button.getAttribute('aria-busy')).toBe('true');
  expect(button.getAttribute('aria-label')).toBe('Disable webcam input');
  expect(t.host.querySelector('[role=status]')?.textContent).toBe('Camera permission was denied.');
  t.setState((s) => {
    s.webcamDisabled = true;
  });
  flush();
  button.click();
  expect(t.toggleWebcam).not.toHaveBeenCalled();
  t.setState((s) => {
    s.webcamSupported = false;
  });
  flush();
  expect(t.host.querySelector('#webcam-toggle')).toBeNull();
});
