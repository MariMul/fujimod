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

// Only a browser page owns a persistent connection. Server renders must not
// reuse process-global or HMR state from a previous request or module version.
const pinned =
  typeof window === 'undefined'
    ? undefined
    : (window as Window & { __fujimodDevToolsPage?: PageState });
let state: PageState | undefined = pinned
  ? (pinned.__fujimodDevToolsPage ?? import.meta.hot?.data.state)
  : undefined;

function createPageState(): PageState {
  // Detached from rendering so both server and browser leave the component's
  // hydration-id sequence untouched.
  return runWithOwner(null, () =>
    createRoot(() => {
      const tools = createDevTools(
        new CameraSources(),
        typeof window !== 'undefined' && isSecureContext && 'usb' in navigator,
        (error) => created.crash(error),
      );
      const created: PageState = {
        tools,
        share: createShareSession(tools),
        crash: () => {},
        started: false,
      };
      return created;
    }),
  );
}

/** One camera connection and share per browser page, across component hot swaps. */
export function pageState(): PageState {
  if (!pinned) return createPageState();
  if (state) return state;
  state = createPageState();
  pinned.__fujimodDevToolsPage = state;
  if (import.meta.env.DEV || import.meta.env.MODE === 'local-preview') {
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
