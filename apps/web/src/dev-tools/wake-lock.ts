import { createEffect, onCleanup } from 'solid-js';

type Lock = { release(): Promise<void> };
type WakeLocks = { request(type: 'screen'): Promise<Lock> };

/**
 * Keep the computer awake while `wanted` is true. Browsers drop the lock while
 * the tab is hidden, so it is requested again whenever the tab is shown.
 * Server and browser create the same effect so hydration keys match.
 */
export function useWakeLock(wanted: () => boolean) {
  let lock: Lock | undefined;
  let want = false;
  let listening = false;
  const locks = () =>
    typeof navigator === 'undefined'
      ? undefined
      : (navigator as Navigator & { wakeLock?: WakeLocks }).wakeLock;
  const acquire = async () => {
    const api = locks();
    if (!api || !want || lock || document.visibilityState !== 'visible') return;
    try {
      const next = await api.request('screen');
      if (want && !lock) lock = next;
      else void next.release();
    } catch {
      /* Refused (battery saver, policy): the share keeps working without it. */
    }
  };
  const release = () => {
    const held = lock;
    lock = undefined;
    void held?.release();
  };
  const shown = () => {
    if (document.visibilityState !== 'visible') return;
    lock = undefined; // Released by the browser while hidden.
    void acquire();
  };
  onCleanup(() => {
    if (listening) document.removeEventListener('visibilitychange', shown);
    want = false;
    release();
  });
  createEffect(wanted, (on) => {
    want = on;
    if (!locks()) return;
    if (!listening) {
      document.addEventListener('visibilitychange', shown);
      listening = true;
    }
    if (on) void acquire();
    else release();
  });
}
