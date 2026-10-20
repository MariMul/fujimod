import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import './tool-deck.css';

export enum DeckTool {
  EnterDebugMode = 'enter-debug-mode',
  EnterServiceMode = 'enter-service-mode',
  ConnectCamera = 'connect-camera',
  Inspector = 'inspector',
  ConnectSimulator = 'connect-simulator',
  Console = 'console',
  Share = 'share',
  LeaveDebugMode = 'leave-debug-mode',
}
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
      <span class="td-description">{props.children}</span>
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
      <a class="td-action td-card" aria-label={props.title} href={props.href}>
        {content()}
      </a>
    </Show>
  );
}
/** Dev Tools start screen: service-mode instructions, connections, and tools. */
export function ToolDeck(props: {
  connected: boolean;
  busy?: boolean;
  usbSupported?: boolean;
  shareLocked?: string;
  onOpen: (tool: DeckTool) => void;
}) {
  const needsConnection = 'Connect a camera or simulator';
  return (
    <section class="td" aria-labelledby="td-title">
      <h1 id="td-title" class="td-heading">
        Dev Tools
      </h1>
      <div class="td-hero">
        <article class="td-tile td-enter">
          <h2>Enter service mode</h2>
          <p>Connect over USB to inspect and control the camera’s internal software.</p>
          <button class="td-cta" onClick={() => props.onOpen(DeckTool.EnterServiceMode)}>
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
              onOpen={() => props.onOpen(DeckTool.ConnectCamera)}
            >
              A camera in service mode or debug mode, over USB.
            </Tile>
            <Tile
              hero
              title="Connect simulator"
              busy={props.busy}
              onOpen={() => props.onOpen(DeckTool.ConnectSimulator)}
            >
              Your saved firmware in the browser. Every tool works with it.
            </Tile>
          </div>
        </div>
      </div>
      <div class="td-grid">
        <Tile title="Console" onOpen={() => props.onOpen(DeckTool.Console)}>
          Send commands to a connected camera or the simulator.
        </Tile>
        <Tile
          title="Inspector"
          locked={props.connected ? undefined : needsConnection}
          onOpen={() => props.onOpen(DeckTool.Inspector)}
        />
        <Tile title="Enter debug mode" onOpen={() => props.onOpen(DeckTool.EnterDebugMode)} />
        <Tile
          title="Share camera"
          locked={props.connected ? props.shareLocked : needsConnection}
          onOpen={() => props.onOpen(DeckTool.Share)}
        >
          Let someone else control your connected camera.
        </Tile>
        <Tile
          title="Leave debug mode"
          locked={props.connected ? undefined : needsConnection}
          onOpen={() => props.onOpen(DeckTool.LeaveDebugMode)}
        >
          Turn the Debug Mode setting off again.
        </Tile>
        <Tile title="Bug reports" href="../bug/">
          Send a report about FujiMod or your camera.
        </Tile>
      </div>
    </section>
  );
}
