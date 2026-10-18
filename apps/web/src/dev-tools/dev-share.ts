import { createEffect, createRoot } from 'solid-js';
import { ConnectionState, ConsoleState, type DevToolsController } from './controller';
import { Level } from './log';

interface Registration {
  url: string;
  camera: { model: string | null; serial: string | null };
}

/** Loaded only by local development pages. The socket owns the registry entry. */
export function startDevShare(
  tools: DevToolsController,
  share: () => Promise<Registration>,
): () => void {
  let socket: WebSocket | undefined;
  let generation = 0;
  let pending = false;
  const withdraw = () => {
    generation++;
    pending = false;
    socket?.close();
    socket = undefined;
  };
  const dispose = createRoot((dispose) => {
    createEffect(
      () => ({ state: tools.connection(), idle: tools.consoleState() === ConsoleState.Idle }),
      ({ state, idle }) => {
        if (state !== ConnectionState.Connected && state !== ConnectionState.Sharing) {
          withdraw();
          return;
        }
        if (!idle || !tools.canShare() || pending) return;
        pending = true;
        const current = generation;
        void share()
          .then((registration) => {
            if (current !== generation) return;
            const url = new URL('/usb-proxy/dev-share', location.href);
            url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
            const connection = new WebSocket(url);
            socket = connection;
            connection.onopen = () => {
              if (current === generation) connection.send(JSON.stringify(registration));
            };
            connection.onmessage = ({ data }) => {
              if (current !== generation) return;
              try {
                const reply = JSON.parse(data);
                if (reply.type === 'registered')
                  tools.connectionMessage('Development share registered');
                else throw Error(reply.message || 'Development share registration failed');
              } catch (error) {
                tools.connectionMessage(String(error), Level.Error);
                connection.close();
              }
            };
            connection.onerror = () => {
              if (current === generation)
                tools.connectionMessage('Development share registry unavailable', Level.Error);
            };
          })
          .catch((error: unknown) => {
            if (current === generation) tools.connectionMessage(String(error), Level.Error);
          });
      },
    );
    return dispose;
  });
  const close = () => withdraw();
  window.addEventListener('pagehide', close);
  return () => {
    window.removeEventListener('pagehide', close);
    dispose();
    withdraw();
  };
}
