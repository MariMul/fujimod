import { createRoot, runWithOwner } from 'solid-js';
import { createDevTools, type DevToolsController } from './controller';
import { createShareSession, type ShareSession } from './share-session';
import { CameraSources } from './sources';

export interface PageState {
  tools: DevToolsController;
  share: ShareSession;
  /** The mounted page's crash reporter; replaced on each mount. */
  crash: (error: unknown) => void;
  started: boolean;
}

// Pinned to the window as well as hot data: a page never holds two of these.
const pinned = globalThis as typeof globalThis & { __fujimodDevToolsPage?: PageState };
let state: PageState | undefined = pinned.__fujimodDevToolsPage ?? import.meta.hot?.data.state;

/**
 * One camera connection and share per page. They live outside the Dev Tools
 * component, so a component hot swap never drops the camera or a share.
 */
export function pageState(): PageState {
  if (state) return state;
  // Detached from the rendering component: a child root would take a
  // hydration id in the browser that the server (state kept) never takes.
  state = runWithOwner(null, () => createRoot(() => {
    const created = { crash: () => {}, started: false } as unknown as PageState;
    created.tools = createDevTools(
      new CameraSources(),
      typeof window !== 'undefined' && isSecureContext && 'usb' in navigator,
      (error) => created.crash(error),
    );
    created.share = createShareSession(created.tools);
    return created;
  }));
  pinned.__fujimodDevToolsPage = state;
  if (
    typeof window !== 'undefined' &&
    (import.meta.env.DEV || import.meta.env.MODE === 'local-preview')
  ) {
    const current = state;
    void import('./dev-share').then(({ startDevShare }) =>
      startDevShare(current.tools, current.share),
    );
  }
  return state;
}

import.meta.hot?.dispose((data) => {
  data.state = state;
});
