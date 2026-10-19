import { ConnectionState, type DevToolsController } from './controller';
import { Level } from './log';
import type { ShareSession } from './share-session';

/**
 * Development pages only. The dev server reloads every open tab when any
 * module changes; a reload during a camera session drops USB and its share
 * mid-command. While a camera is connected or shared, hold the reload (Vite
 * awaits `vite:beforeFullReload` listeners) and let it through afterwards.
 */
export function holdReloadsDuringCameraSessions(tools: DevToolsController, share: ShareSession) {
  const hot = import.meta.hot;
  if (!hot) return;
  const busy = () => {
    const connection = tools.connection();
    return (
      share.active() ||
      ![
        ConnectionState.Disconnected,
        ConnectionState.Unsupported,
        ConnectionState.Loading,
      ].includes(connection)
    );
  };
  let held = false;
  hot.on('vite:beforeFullReload', async () => {
    if (!busy()) return;
    if (!held)
      tools.connectionMessage('Page update waiting until the camera disconnects', Level.Info);
    held = true;
    while (busy()) await new Promise((resolve) => setTimeout(resolve, 500));
  });
}
