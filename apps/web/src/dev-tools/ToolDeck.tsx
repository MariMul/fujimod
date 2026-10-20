import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import './tool-deck.css';

export enum DeckTool {
  EnterDebugMode = 'enter-debug-mode',
  Console = 'console',
  Share = 'share',
  LeaveDebugMode = 'leave-debug-mode',
}
function Card(props: {
  title: string;
  locked?: string;
  onOpen?: () => void;
  href?: string;
  primary?: boolean;
  children?: JSX.Element;
}) {
  return (
    <article
      class={{
        'td-card': true,
        'td-primary': props.primary,
        'td-locked': Boolean(props.locked),
      }}
    >
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
              {props.primary ? 'Show instructions' : 'Open'}
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
/** Dev Tools start screen: one card per tool, locked tools say what they need. */
export function ToolDeck(props: {
  debugCamera: boolean;
  shareLocked?: string;
  onOpen: (tool: DeckTool) => void;
}) {
  const needsDebug = 'Needs a camera in debug mode';
  return (
    <section class="td" aria-labelledby="td-title">
      <h1 id="td-title" class="td-heading">
        Dev Tools
      </h1>
      <div class="td-grid">
        <Card title="Enter debug mode" primary onOpen={() => props.onOpen(DeckTool.EnterDebugMode)}>
          <p>
            In debug mode the camera works normally, except its USB connection can inspect and
            control the camera’s software. USB must be plugged in twice for regular file transfer.
          </p>
          <p class="td-note">Required for other Dev Tools.</p>
        </Card>
        <Card title="Console" onOpen={() => props.onOpen(DeckTool.Console)}>
          <p>Send commands to a camera in debug mode or to the simulator.</p>
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
        <Card title="Emulator" href="../">
          <p>Run your saved camera firmware in the browser.</p>
        </Card>
        <Card title="Bug reports" href="../bug/">
          <p>Send a report about FujiMod or your camera.</p>
        </Card>
      </div>
    </section>
  );
}
