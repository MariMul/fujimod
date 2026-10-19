import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { getRequestEvent, HydrationScript } from '@solidjs/web';
export default function Document(props: { children: JSX.Element }) {
  const path = new URL(getRequestEvent()?.request.url ?? window.location.href).pathname;
  const bug = /^\/bug(?:\/|$)/.test(path),
    tools = /^\/dev-tools(?:\/|$)/.test(path),
    setup = /^\/setup(?:\/|$)/.test(path);
  return (
    <html lang="en">
      <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width,initial-scale=1" />
        <base href={import.meta.env.BASE_URL} />
        <meta
          name="description"
          content={
            bug
              ? 'Retrieve and prepare a private camera bug report locally.'
              : setup
                ? 'Put your camera in service mode and connect it over USB, step by step.'
                : tools
                ? 'FujiMod USB console and protocol logs.'
                : 'Try your camera firmware in the browser and build Fujimod SD boot files.'
          }
        />
        <meta name="referrer" content="no-referrer" />
        <Show when={import.meta.env.DEV && !bug}>
          <meta name="fujimod-local-reports" content="/__simulator-reports" />
          <script src="/__simulator-reports.js" />
          <script src="/__browser-diagnostics.js" />
        </Show>
        <title>
          {bug
            ? 'Report a camera bug · FujiMod'
            : setup
              ? 'Set up USB access · FujiMod'
              : tools
                ? 'Dev Tools · FujiMod'
                : 'Fujimod'}
        </title>
        <link rel="icon" href="data:," />
        <HydrationScript />
      </head>
      <body>{props.children}</body>
    </html>
  );
}
