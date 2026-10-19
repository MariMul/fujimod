import { expect, test } from 'vite-plus/test';
import { pageState } from './page-state';

function release(state: ReturnType<typeof pageState>) {
  state.share.dispose();
  state.tools.dispose();
}

test('server renders create independent page lifetimes', () => {
  expect(typeof window).toBe('undefined');
  const first = pageState();
  first.started = true;
  const second = pageState();
  try {
    expect(second).not.toBe(first);
    expect(second.tools).not.toBe(first.tools);
    expect(second.share).not.toBe(first.share);
    expect(second.started).toBe(false);
    expect(second.share.progress()).toBeUndefined();
    expect(second.share.activity()).toBeUndefined();
  } finally {
    release(first);
    release(second);
  }
});

test('server renders ignore a stale browser-state pin on the server global', () => {
  const old = Reflect.get(globalThis, '__fujimodDevToolsPage');
  const stale = { started: true, share: {} };
  Reflect.set(globalThis, '__fujimodDevToolsPage', stale);
  const state = pageState();
  try {
    expect(state).not.toBe(stale);
    expect(state.started).toBe(false);
    expect(state.share.progress()).toBeUndefined();
    expect(Reflect.get(globalThis, '__fujimodDevToolsPage')).toBe(stale);
  } finally {
    release(state);
    if (old === undefined) Reflect.deleteProperty(globalThis, '__fujimodDevToolsPage');
    else Reflect.set(globalThis, '__fujimodDevToolsPage', old);
  }
});
