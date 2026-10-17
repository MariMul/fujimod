// @vitest-environment jsdom
import { createSignal, flush, onSettled } from 'solid-js';
import { render } from '@solidjs/web';
import { expect, test } from 'vite-plus/test';

test('TSX views track shared state and dispose subscriptions on unmount', () => {
  const [connected, setConnected] = createSignal(false);
  const host = document.createElement('div');
  document.body.append(host);
  let notify = () => {};
  let subscribed = false;
  function Connection() {
    onSettled(() => {
      subscribed = true;
      notify = () => setConnected(true);
      return () => {
        subscribed = false;
        notify = () => {};
      };
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
  flush();
  try {
    const button = host.querySelector('button')!;
    const output = host.querySelector('output')!;
    expect(output.textContent).toBe('Disconnected');
    expect(button.disabled).toBe(false);
    button.click();
    flush();
    expect(output.textContent).toBe('Connected');
    expect(button.disabled).toBe(true);
    setConnected(false);
    flush();
    expect(output.textContent).toBe('Disconnected');
    notify();
    flush();
    expect(output.textContent).toBe('Connected');
    expect(button.disabled).toBe(true);
  } finally {
    dispose();
    host.remove();
  }
  expect(subscribed).toBe(false);
  expect(host.childNodes.length).toBe(0);
});
