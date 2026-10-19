import { expect, test, vi } from 'vite-plus/test';

const { startDevShare } = vi.hoisted(() => ({ startDevShare: vi.fn(() => () => {}) }));
vi.mock('./dev-share', () => ({ startDevShare }));

test('browser component remounts keep one page state and development share', async () => {
  vi.stubGlobal('isSecureContext', false);
  const { pageState } = await import('./page-state');
  const first = pageState();
  first.started = true;
  try {
    const second = pageState();
    expect(second).toBe(first);
    expect(second.started).toBe(true);
    expect(second.tools).toBe(first.tools);
    expect(second.share).toBe(first.share);
    await vi.waitFor(() =>
      expect(startDevShare).toHaveBeenCalledExactlyOnceWith(first.tools, first.share),
    );
  } finally {
    first.share.dispose();
    first.tools.dispose();
    Reflect.deleteProperty(window, '__fujimodDevToolsPage');
    vi.unstubAllGlobals();
  }
});
