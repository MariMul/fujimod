import { createEffect, createRoot } from 'solid-js';
import { CameraTarget } from './camera';
import { ConnectionState, ConsoleState, type DevToolsController } from './controller';
import { Level } from './log';
import type { ShareSession } from './share-session';
import { holdReloadsDuringCameraSessions } from './reload-hold';

/**
 * Loaded only by local development pages. Shares a connected camera
 * automatically and keeps its registration (link, camera, online state and
 * battery) current in the relay's registry, where host tools find it. The
 * socket owns the registry entry.
 */
export function startDevShare(tools: DevToolsController, share: ShareSession): () => void {
  holdReloadsDuringCameraSessions(tools, share);
  let socket: WebSocket | undefined;
  // Stop sharing turns automatic sharing off until the camera is reconnected.
  let suspended = false;
  const close = () => {
    socket?.close();
    socket = undefined;
  };
  const open = () => {
    const url = new URL('/usb-proxy/dev-share', location.href);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    const connection = new WebSocket(url);
    socket = connection;
    let announced = false;
    connection.onopen = () => {
      const registration = share.registration();
      if (connection === socket && registration) connection.send(JSON.stringify(registration));
    };
    connection.onmessage = ({ data }) => {
      if (connection !== socket) return;
      try {
        const reply = JSON.parse(data);
        if (reply.type !== 'registered')
          throw Error(reply.message || 'Development share registration failed');
        if (!announced) tools.connectionMessage('Development share registered');
        announced = true;
      } catch (error) {
        tools.connectionMessage(String(error), Level.Error);
        connection.close();
      }
    };
    connection.onerror = () => {
      if (connection === socket)
        tools.connectionMessage('Development share registry unavailable', Level.Error);
    };
  };
  const dispose = createRoot((dispose) => {
    createEffect(
      () => share.registration(),
      (registration) => {
        if (!registration) return close();
        if (!socket) return open();
        if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(registration));
      },
    );
    createEffect(
      () => ({
        state: tools.connection(),
        idle: tools.consoleState() === ConsoleState.Idle,
        sharing: share.sharing(),
      }),
      ({ state, idle, sharing }) => {
        if (state !== ConnectionState.Connected && state !== ConnectionState.Sharing) {
          suspended = false;
          return;
        }
        if (sharing) return;
        // Only physical cameras: a simulator registered here would receive camera jobs.
        const physical = tools.target() === CameraTarget.Usb;
        if (
          state === ConnectionState.Connected &&
          physical &&
          tools.canShare() &&
          idle &&
          !suspended
        ) {
          void share.share().catch((error: unknown) => {
            tools.connectionMessage(String(error), Level.Error);
          });
        }
      },
    );
    let wasSharing = false;
    createEffect(
      () => share.sharing(),
      (sharing) => {
        // A share that ends while the camera stays connected was stopped on purpose.
        if (wasSharing && !sharing && tools.connection() === ConnectionState.Connected)
          suspended = true;
        wasSharing = sharing;
      },
    );
    return dispose;
  });
  // An unloading page may never run another effect: withdraw synchronously.
  window.addEventListener('pagehide', close);
  document.addEventListener('freeze', close);
  return () => {
    window.removeEventListener('pagehide', close);
    document.removeEventListener('freeze', close);
    dispose();
    close();
  };
}
