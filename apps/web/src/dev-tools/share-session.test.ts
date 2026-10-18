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
