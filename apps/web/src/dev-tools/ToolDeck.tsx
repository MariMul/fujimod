import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import './tool-deck.css';

function Tile(props: {
  title: string;
  locked?: string;
  busy?: boolean;
  hero?: boolean;
  onOpen?: () => void;
  href?: string;
  children?: JSX.Element;
}) {
  const content = () => (
    <>
      <span class="td-title">{props.title}</span>
      <Show when={props.children}>
        <span class="td-description">{props.children}</span>
      </Show>
      <Show when={props.locked}>
        <span class="td-lock">
          <span aria-hidden="true">■</span> {props.locked}
        </span>
      </Show>
      <Show when={props.busy}>
        <span class="td-status">Connecting…</span>
      </Show>
    </>
  );
  return (
    <Show
      when={props.href}
      fallback={
        <button
          type="button"
          class={{ 'td-action': true, 'td-tile': props.hero, 'td-card': !props.hero }}
          aria-label={props.title}
          aria-busy={props.busy ? 'true' : 'false'}
          disabled={Boolean(props.locked) || props.busy}
          onClick={() => props.onOpen?.()}
        >
          {content()}
        </button>
      }
    >
      <a
        class={{ 'td-action': true, 'td-tile': props.hero, 'td-card': !props.hero }}
        aria-label={props.title}
        href={props.href}
      >
        {content()}
      </a>
    </Show>
  );
}
/** Dev Tools start screen, shown until a camera is connected. */
export function ToolDeck(props: {
  busy?: boolean;
  usbSupported?: boolean;
  onServiceMode: () => void;
  onConnect: () => void;
}) {
  return (
    <section class="td" aria-labelledby="td-title">
      <h1 id="td-title" class="td-heading">
        Dev Tools
      </h1>
      <div class="td-hero">
        <article class="td-tile td-enter">
          <h2>Enter service mode</h2>
          <p>Service mode exposes the camera’s ff80 USB connection.</p>
          <p class="td-note">
            Switching the camera off in service mode restores factory settings. Remove the battery
            to leave service mode.
          </p>
          <button class="td-cta" onClick={() => props.onServiceMode()}>
            Show instructions
          </button>
        </article>
        <div class="td-connect-group">
          <span class="td-connect-label">Connect a camera</span>
          <div class="td-connects">
            <Tile
              hero
              title="Connect camera"
              locked={
                props.usbSupported
                  ? undefined
                  : 'This browser can’t connect to USB. Use Chrome or Edge.'
              }
              busy={props.busy}
              onOpen={() => props.onConnect()}
            >
              A camera in service mode or debug mode, over USB.
            </Tile>
            <Tile hero title="Open simulator" href="../?scenario=x70-lcd#workspace" />
          </div>
        </div>
      </div>
      <a class="td-footer-link" href="../bug/">
        Bug reports
      </a>
    </section>
  );
}
