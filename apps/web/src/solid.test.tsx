// @vitest-environment jsdom
import { createSignal, onCleanup } from 'solid-js';
import { render } from 'solid-js/web';
import { expect, test } from 'vite-plus/test';

test('TSX views track shared state and dispose subscriptions on unmount', () => {
  const [connected, setConnected] = createSignal(false);
  const host = document.createElement('div');
  document.body.append(host);
  let notify = () => {};
  let subscribed = false;
  function Connection() {
    subscribed = true;
    notify = () => setConnected(true);
    onCleanup(() => {
      subscribed = false;
      notify = () => {};
    });
    return (
      <>
        <button disabled={connected()} onClick={() => setConnected(true)}>
          Connect
        </button>
        <output>{connected() ? 'Connected' : 'Disconnected'}</output>
      </>
    );
  }
  const dispose = render(() => <Connection />, host);
  try {
    const button = host.querySelector('button')!;
    const output = host.querySelector('output')!;
    expect(output.textContent).toBe('Disconnected');
    expect(button.disabled).toBe(false);
    button.click();
    expect(output.textContent).toBe('Connected');
    expect(button.disabled).toBe(true);
    setConnected(false);
    notify();
    expect(output.textContent).toBe('Connected');
    expect(button.disabled).toBe(true);
  } finally {
    dispose();
    host.remove();
  }
  expect(subscribed).toBe(false);
  expect(host.childNodes.length).toBe(0);
});
