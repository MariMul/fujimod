import { createEffect, onCleanup } from 'solid-js';

type Lock = { release(): Promise<void> };
type WakeLocks = { request(type: 'screen'): Promise<Lock> };

/**
 * Keep the computer awake while `wanted` is true. Browsers drop the lock while
 * the tab is hidden, so it is requested again whenever the tab is shown.
 */
export function useWakeLock(wanted: () => boolean) {
  const locks = (navigator as Navigator & { wakeLock?: WakeLocks }).wakeLock;
  if (!locks) return;
  let lock: Lock | undefined;
  let want = false;
  const acquire = async () => {
    if (!want || lock || document.visibilityState !== 'visible') return;
    try {
      const next = await locks.request('screen');
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
  document.addEventListener('visibilitychange', shown);
  onCleanup(() => {
    document.removeEventListener('visibilitychange', shown);
    want = false;
    release();
  });
  createEffect(wanted, (on) => {
    want = on;
    if (on) void acquire();
    else release();
  });
}
