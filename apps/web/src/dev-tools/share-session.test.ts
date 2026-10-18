import { expect, test, vi } from 'vite-plus/test';
import { browserEnvironment } from './share-session';

test('server rendering does not install browser listeners', () => {
  expect(typeof document).toBe('undefined');
  const environment = browserEnvironment();
  const listener = vi.fn();
  expect(environment.visible()).toBe(false);
  environment.onVisible(listener)();
  environment.onLeave(listener)();
  expect(listener).not.toHaveBeenCalled();
});

test('server page state never starts the development share service', async () => {
  const { pageState } = await import('./page-state');
  const state = pageState();
  expect(state.share.active()).toBe(false);
  // The browser service is a dynamic import; let a mistaken launch surface.
  await new Promise((resolve) => setTimeout(resolve, 0));
  state.share.dispose();
  state.tools.dispose();
});
