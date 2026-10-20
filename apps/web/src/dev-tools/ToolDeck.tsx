import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import './tool-deck.css';

export enum DeckTool {
  EnterDebugMode = 'enter-debug-mode',
  ConnectDebugCamera = 'connect-debug-camera',
  ConnectSimulator = 'connect-simulator',
  Console = 'console',
  Share = 'share',
  LeaveDebugMode = 'leave-debug-mode',
}
function Card(props: {
  title: string;
  locked?: string;
  onOpen?: () => void;
  href?: string;
  children?: JSX.Element;
}) {
  return (
    <article class={{ 'td-card': true, 'td-locked': Boolean(props.locked) }}>
      <h2>{props.title}</h2>
      {props.children}
      <Show
        when={!props.locked}
        fallback={
          <p class="td-lock">
            <span aria-hidden="true">■</span> {props.locked}
          </p>
        }
      >
        <Show
          when={props.href}
          fallback={
            <button class="td-open" onClick={() => props.onOpen?.()}>
              Open
            </button>
          }
        >
          <a class="td-open" href={props.href}>
            Open
          </a>
        </Show>
      </Show>
    </article>
  );
}
/** Dev Tools start screen: two debug-mode entry points, the simulator, and tools. */
export function ToolDeck(props: {
  debugCamera: boolean;
  busy?: boolean;
  usbSupported?: boolean;
  shareLocked?: string;
  onOpen: (tool: DeckTool) => void;
}) {
  const needsDebug = 'Needs a camera in debug mode';
  const busyLabel = (label: string) => (props.busy ? 'Connecting…' : label);
  return (
    <section class="td" aria-labelledby="td-title">
      <h1 id="td-title" class="td-heading">
        Dev Tools
      </h1>
      <div class="td-hero">
        <article class="td-tile td-enter">
          <h2>Enter debug mode</h2>
          <p>
            Set up a camera that isn’t in debug mode yet. In debug mode the camera works normally,
            except its USB connection can inspect and control the camera’s software.
          </p>
          <button
            class="td-cta td-cta-outline"
            onClick={() => props.onOpen(DeckTool.EnterDebugMode)}
          >
            Show instructions
          </button>
        </article>
        <article class="td-tile td-connect">
          <h2>Connect a debug-mode camera</h2>
          <p>
            Already set up? Connect a camera that’s in debug mode over USB to unlock the tools
            below.
          </p>
          <Show
            when={props.usbSupported}
            fallback={<p class="td-lock">This browser can’t connect to USB. Use Chrome or Edge.</p>}
          >
            <button
              class="td-cta td-cta-fill"
              disabled={props.busy}
              onClick={() => props.onOpen(DeckTool.ConnectDebugCamera)}
            >
              {busyLabel('Connect camera')}
            </button>
          </Show>
        </article>
      </div>
      <article class="td-sim">
        <div>
          <h2>Connect a simulated camera</h2>
          <p>Run any Dev Tool against your saved firmware in the browser — no camera needed.</p>
        </div>
        <button
          class="td-cta td-cta-outline"
          disabled={props.busy}
          onClick={() => props.onOpen(DeckTool.ConnectSimulator)}
        >
          {busyLabel('Connect simulator')}
        </button>
      </article>
      <div class="td-grid">
        <Card title="Console" onOpen={() => props.onOpen(DeckTool.Console)}>
          <p>Send commands to a connected camera or the simulator.</p>
        </Card>
        <Card
          title="Share camera"
          locked={props.debugCamera ? props.shareLocked : needsDebug}
          onOpen={() => props.onOpen(DeckTool.Share)}
        >
          <p>Let someone else control your connected camera.</p>
        </Card>
        <Card
          title="Leave debug mode"
          locked={props.debugCamera ? undefined : needsDebug}
          onOpen={() => props.onOpen(DeckTool.LeaveDebugMode)}
        >
          <p>Turn the Debug Mode setting off again.</p>
        </Card>
        <Card title="Bug reports" href="../bug/">
          <p>Send a report about FujiMod or your camera.</p>
        </Card>
      </div>
    </section>
  );
}
