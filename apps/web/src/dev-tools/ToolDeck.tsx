import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import './tool-deck.css';

export enum DeckTool {
  EnterDebugMode = 'enter-debug-mode',
  ConnectDebugCamera = 'connect-debug-camera',
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
/** Dev Tools start screen: debug-mode instructions, connections, and tools. */
export function ToolDeck(props: {
  debugCamera: boolean;
  busy?: boolean;
  usbSupported?: boolean;
  shareLocked?: string;
  onOpen: (tool: DeckTool) => void;
}) {
  const needsDebug = 'Needs a camera in debug mode';
  return (
    <section class="td" aria-labelledby="td-title">
      <h1 id="td-title" class="td-heading">
        Dev Tools
      </h1>
      <div class="td-hero">
        <article class="td-tile td-enter">
          <h2>Enter debug mode</h2>
          <p>
            In debug mode, the camera operates normally, except its USB connection can be used to
            inspect and control the camera’s internal software. USB must be plugged in twice for
            regular file transfer.
          </p>
          <p class="td-note">Required for other Dev Tools.</p>
          <button class="td-cta" onClick={() => props.onOpen(DeckTool.EnterDebugMode)}>
            Show instructions
          </button>
        </article>
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
            onOpen={() => props.onOpen(DeckTool.ConnectDebugCamera)}
          >
            A camera already in debug mode, over USB.
          </Tile>
          <Tile hero title="Open simulator" href="../?scenario=x70-lcd#workspace" />
        </div>
      </div>
      <div class="td-grid">
        <Tile title="Console" onOpen={() => props.onOpen(DeckTool.Console)}>
          Send commands to a connected camera.
        </Tile>
        <Tile
          title="Share camera"
          locked={props.debugCamera ? props.shareLocked : needsDebug}
          onOpen={() => props.onOpen(DeckTool.Share)}
        >
          Let someone else control your connected camera.
        </Tile>
        <Tile
          title="Leave debug mode"
          locked={props.debugCamera ? undefined : needsDebug}
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
