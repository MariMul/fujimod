import { For, Show, createEffect, createMemo, createSignal, onSettled, untrack } from 'solid-js';
import { CameraTarget, type CameraBackend } from './camera';
import { ConnectionState, ConsoleState, LineEnding, createDevTools } from './controller';
import { Direction, Layer, Level, exportLog, hex, type LogEntry } from './log';
import { remoteControlUrl, useShareSheet, sharingConfigured } from './sharing';
import { createShareSession } from './share-session';
import { pageState } from './page-state';
import { ProblemAlert, RemoteUseStrip, SharingChip } from './SharingStatus';
import { ToolActivity } from './ToolActivity';
import './sharing.css';
import { LocalReports } from '../ui/local-reports';
import { CameraSources } from './sources';
import '../style.css';
import './style.css';
declare const __FUJIMOD_SIMULATOR_BUILD__: string;
const layers = [Layer.Console, Layer.App80, Layer.Ff80, Layer.Usb];
const levels = [Level.Error, Level.Warning, Level.Info, Level.Debug, Level.Trace];
const connectionNames: Record<ConnectionState, string> = {
  [ConnectionState.Loading]: 'Loading USB',
  [ConnectionState.Disconnected]: 'Disconnected',
  [ConnectionState.Choosing]: 'Choose a camera',
  [ConnectionState.Selecting]: 'Choose an interface',
  [ConnectionState.Opening]: 'Opening session',
  [ConnectionState.Connected]: 'Connected',
  [ConnectionState.Sharing]: 'Sharing',
  [ConnectionState.Closing]: 'Disconnecting',
  [ConnectionState.Blocked]: 'Connection interrupted',
  [ConnectionState.Unsupported]: 'WebUSB unavailable',
};
function LogRow(props: { entry: LogEntry }) {
  const [expanded, setExpanded] = createSignal(false);
  const bytes = createMemo(() => props.entry.data?.length ?? 0);
  return (
    <div
      class={`dt-entry level-${props.entry.level.toLowerCase()} ${props.entry.layer === Layer.Console ? 'is-console' : ''}`}
    >
      <button
        class="dt-row"
        aria-expanded={expanded() ? 'true' : 'false'}
        onClick={() => {
          setExpanded((value) => !value);
        }}
      >
        <span
          class="dt-direction"
          aria-label={
            props.entry.direction === Direction.Out
              ? 'To camera'
              : props.entry.direction === Direction.In
                ? 'From camera'
                : 'Host'
          }
        >
          {props.entry.direction}
        </span>
        <span class="dt-layer">{props.entry.layer}</span>
        <span class="dt-message">{props.entry.message.trimEnd()}</span>
        <span class="dt-size">{bytes() ? `${bytes().toLocaleString()} B` : ''}</span>
        <time class="dt-time" datetime={new Date(props.entry.time).toISOString()}>
          {new Date(props.entry.time).toLocaleTimeString(undefined, { hour12: false })}
        </time>
        <span class="dt-expand" aria-hidden="true">
          {expanded() ? '−' : '+'}
        </span>
      </button>
      <Show when={expanded()}>
        <div class="dt-detail">
          <div>
            {props.entry.level} · {props.entry.correlation ?? 'Host'} ·{' '}
            {new Date(props.entry.time).toISOString()}
          </div>
          <Show when={props.entry.detail}>
            <pre>{props.entry.detail}</pre>
          </Show>
          <Show when={bytes()}>
            <pre>{hex(props.entry.data!)}</pre>
          </Show>
        </div>
      </Show>
    </div>
  );
}
export default function DevTools(props: {
  backend?: CameraBackend;
  supported?: boolean;
  connectOnMount?: boolean;
}) {
  // Tests pass their own backend and get their own instances; the page keeps one
  // camera connection and share for its lifetime, across component hot swaps.
  const owned = untrack(() => props.backend) !== undefined;
  const page = owned ? undefined : pageState();
  const tools =
    page?.tools ??
    createDevTools(
      untrack(() => props.backend) ?? new CameraSources(),
      untrack(() => props.supported) ??
        (typeof window !== 'undefined' && isSecureContext && 'usb' in navigator),
      (error) => reports.crash(error),
    );
  const share = page?.share ?? createShareSession(tools);
  const context = () => ({
    event:
      tools.connection() === ConnectionState.Blocked ? ('crash' as const) : ('feedback' as const),
    camera: tools.camera()?.firmware,
    diagnostics: tools.reportContext(),
    modBuild: 'Stock firmware (no mods)',
    simulatorBuild: __FUJIMOD_SIMULATOR_BUILD__,
  });
  const reports = new LocalReports('dev-tools', context, tools.errorReport);
  if (page) page.crash = (error) => reports.crash(error);
  const [activeLayers, setActiveLayers] = createSignal<ReadonlySet<Layer>>(
    new Set([Layer.Console, Layer.App80, Layer.Ff80]),
  );
  const [activeLevels, setActiveLevels] = createSignal<ReadonlySet<Level>>(new Set(levels));
  const [target, setTarget] = createSignal(CameraTarget.Usb);
  const sharedUrl = share.url;
  const [sharingBusy, setSharingBusy] = createSignal(false);
  const [toast, setToast] = createSignal('');
  const [copyFallback, setCopyFallback] = createSignal(false);
  let shareDialog!: HTMLDialogElement;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function notice(text: string) {
    setToast(text);
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => setToast(''), 4500);
  }
  function shareCamera() {
    if (sharingBusy()) return;
    share.unlockAudio();
    const sheet = useShareSheet();
    const url = share.address();
    // Invoke the native sheet inside the camera button's activation, before I/O.
    const delivery = sheet ? navigator.share({ title: 'FujiMod camera', url }) : undefined;
    delivery?.catch((error: unknown) => {
      if (!(error instanceof Error && error.name === 'AbortError')) notice(String(error));
    });
    setSharingBusy(true);
    void (async () => {
      try {
        await share.share();
        if (!sheet) {
          try {
            await navigator.clipboard.writeText(url);
            notice('Share link copied');
            shareDialog.close();
          } catch {
            setCopyFallback(true);
          }
        } else shareDialog.close();
      } catch (error) {
        notice(error instanceof Error ? error.message : String(error));
      } finally {
        setSharingBusy(false);
      }
    })();
  }
  const [draft, setDraft] = createSignal('');
  const [ending, setEnding] = createSignal(LineEnding.Lf);
  const [follow, setFollow] = createSignal(true);
  const [selectedInterface, setSelectedInterface] = createSignal(0);
  let viewport!: HTMLDivElement;
  let input!: HTMLInputElement;
  let history: string[] = [],
    historyIndex = 0,
    savedDraft = '';
  const entries = createMemo(() =>
    tools
      .entries()
      .filter(
        (entry) =>
          (entry.layer === Layer.Connection || activeLayers().has(entry.layer)) &&
          activeLevels().has(entry.level),
      ),
  );
  const canSend = createMemo(
    () =>
      tools.connection() === ConnectionState.Connected &&
      tools.consoleState() === ConsoleState.Idle,
  );
  onSettled(() => {
    const unlisten = reports.listen();
    let disposed = false;
    let stopDevShare: (() => void) | undefined;
    // The page-lifetime state starts its own development share; owned instances (tests) do here.
    if (owned && (import.meta.env.DEV || import.meta.env.MODE === 'local-preview')) {
      void import('./dev-share').then(({ startDevShare }) => {
        if (!disposed) stopDevShare = startDevShare(tools, share);
      });
    }
    // A hot-swapped component finds the camera already started (and maybe shared).
    if (page?.started) return () => cleanup();
    if (page) page.started = true;
    void tools.start().then(async () => {
      if (props.connectOnMount) {
        await tools.connect(CameraTarget.Usb);
        return;
      }
      try {
        if (remoteControlUrl(location.href)) {
          setTarget(CameraTarget.Remote);
          await tools.connect(CameraTarget.Remote);
          return;
        }
      } catch (error) {
        tools.connectionMessage(
          error instanceof Error ? error.message : String(error),
          Level.Error,
        );
        return;
      }
    });
    function cleanup() {
      disposed = true;
      stopDevShare?.();
      unlisten();
      if (toastTimer) clearTimeout(toastTimer);
      // Page-lifetime camera and share outlive the component; owned ones end with it.
      if (owned) {
        share.dispose();
        tools.dispose();
      }
    }
    return () => cleanup();
  });
  createEffect(
    () => ({ count: entries().length, follow: follow() }),
    ({ follow: enabled }) => {
      if (enabled)
        queueMicrotask(() => {
          if (viewport) viewport.scrollTop = viewport.scrollHeight;
        });
    },
  );
  function toggleLayer(layer: Layer) {
    setActiveLayers((previous) => {
      const next = new Set(previous);
      if (next.has(layer)) next.delete(layer);
      else next.add(layer);
      return next;
    });
  }
  function toggleLevel(level: Level) {
    setActiveLevels((previous) => {
      const next = new Set(previous);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });
  }
  function send(event: SubmitEvent) {
    event.preventDefault();
    const text = draft();
    if (!canSend() || !text.trim()) return;
    if (history.at(-1) !== text) history = [...history.slice(-99), text];
    historyIndex = history.length;
    savedDraft = '';
    setDraft('');
    input.focus();
    void tools.send(text, ending()).then((accepted) => {
      if (!accepted) setDraft((current) => current || text);
    });
  }
  function recall(event: KeyboardEvent) {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
    if (historyIndex === history.length) savedDraft = input.value;
    event.preventDefault();
    historyIndex = Math.max(
      0,
      Math.min(history.length, historyIndex + (event.key === 'ArrowUp' ? -1 : 1)),
    );
    setDraft(history[historyIndex] ?? savedDraft);
  }
  function download() {
    const url = URL.createObjectURL(
      new Blob([exportLog(tools.entries())], { type: 'application/json' }),
    );
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `fujimod-usb-${new Date().toISOString().replaceAll(':', '-')}.json`;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  return (
    <div class="shell dev-tools">
      <header>
        <a href="../" aria-label="Fujimod">
          <img src="../brand.png" alt="Fujimod" width="110" height="28" />
        </a>
        <nav aria-label="Pages">
          <a href="../">Emulator</a>
          <a href="./" aria-current="page">
            Dev Tools
          </a>
        </nav>
        <div class="dt-connection">
          <SharingChip session={share} onOpen={() => shareDialog.showModal()} />
          <button
            class="pill"
            hidden={share.sharing()}
            disabled={
              !sharingConfigured() ||
              !tools.canShare() ||
              ![ConnectionState.Connected, ConnectionState.Sharing].includes(tools.connection())
            }
            onClick={() => shareDialog.showModal()}
          >
            Share
          </button>
          <select
            aria-label="Connection target"
            value={target()}
            disabled={
              ![ConnectionState.Disconnected, ConnectionState.Unsupported].includes(
                tools.connection(),
              )
            }
            onChange={(event) => setTarget(event.currentTarget.value as CameraTarget)}
          >
            <option value={CameraTarget.Usb}>Camera · USB</option>
            <Show when={target() === CameraTarget.Remote}>
              <option value={CameraTarget.Remote}>Shared camera</option>
            </Show>
          </select>
          <span class="dt-status" role="status">
            <i class={tools.connection() === ConnectionState.Connected ? 'online' : ''} />
            {tools.connection() === ConnectionState.Unsupported && target() !== CameraTarget.Usb
              ? 'Disconnected'
              : connectionNames[tools.connection()]}
          </span>
          <Show
            when={
              tools.connection() === ConnectionState.Disconnected ||
              (tools.connection() === ConnectionState.Unsupported && target() !== CameraTarget.Usb)
            }
            fallback={
              <Show
                when={
                  tools.connection() === ConnectionState.Connected ||
                  tools.connection() === ConnectionState.Blocked ||
                  tools.connection() === ConnectionState.Sharing ||
                  tools.connection() === ConnectionState.Choosing ||
                  tools.connection() === ConnectionState.Opening
                }
              >
                <button
                  class="pill"
                  onClick={() => {
                    share.stop();
                    tools.disconnect();
                  }}
                >
                  {tools.connection() === ConnectionState.Choosing ||
                  tools.connection() === ConnectionState.Opening
                    ? 'Cancel'
                    : 'Disconnect'}
                </button>
              </Show>
            }
          >
            <button
              class="pill dt-primary"
              onClick={() => {
                share.unlockAudio();
                void tools.connect(target());
              }}
            >
              {target() === CameraTarget.Remote ? 'Reconnect shared camera' : 'Connect camera'}
            </button>
          </Show>
        </div>
      </header>
      <RemoteUseStrip session={share} />
      <ProblemAlert session={share} />
      <dialog
        ref={(element) => {
          shareDialog = element;
        }}
        class="dt-share-dialog"
        aria-label="Share camera"
      >
        <div class="dt-share-heading">
          <h2>Share camera</h2>
          <button
            class="dt-text-button"
            aria-label="Close share dialog"
            onClick={() => shareDialog.close()}
          >
            ✕
          </button>
        </div>
        <strong class="dt-share-warning">
          Anyone with a share link will be able to control your USB device.
        </strong>
        <button
          class="dt-share-camera"
          disabled={
            sharingBusy() ||
            (tools.connection() === ConnectionState.Connected &&
              tools.consoleState() !== ConsoleState.Idle)
          }
          onClick={shareCamera}
        >
          {tools.camera()?.name ?? 'Camera'}
        </button>
        <Show when={copyFallback()}>
          <input
            aria-label="Share link"
            value={sharedUrl()}
            readonly
            onFocus={(event) => event.currentTarget.select()}
          />
        </Show>
        <Show when={share.controllers().size > 0}>
          <ul class="dt-share-controllers" aria-label="Connected controllers">
            <For each={[...share.controllers().values()]}>
              {(reason) => <li>{reason || 'Remote controller'}</li>}
            </For>
          </ul>
        </Show>
        <Show when={sharedUrl()}>
          <button
            class="dt-text-button dt-share-stop"
            onClick={() => {
              share.stop();
              shareDialog.close();
            }}
          >
            Stop sharing
          </button>
        </Show>
      </dialog>
      <Show when={toast()}>
        <div class="dt-share-toast" role="status">
          {toast()}
        </div>
      </Show>
      <main class="dt-workspace">
        <h1 class="dt-title">Dev Tools</h1>
        <Show when={tools.connection() === ConnectionState.Selecting}>
          <form
            class="dt-interface"
            onSubmit={(event) => {
              event.preventDefault();
              void tools.open(selectedInterface());
            }}
          >
            <label for="usb-interface">USB interface</label>
            <select
              id="usb-interface"
              value={selectedInterface()}
              onChange={(event) => setSelectedInterface(Number(event.currentTarget.value))}
            >
              <For each={tools.camera()?.interfaces ?? []}>
                {(choice) => <option value={choice.id}>{choice.label}</option>}
              </For>
            </select>
            <button class="pill dt-primary" type="submit">
              Open session
            </button>
            <button class="pill" type="button" onClick={() => tools.disconnect()}>
              Cancel
            </button>
          </form>
        </Show>
        <div class="dt-console">
          <div class="dt-toolbar">
            <div class="dt-filters" role="group" aria-label="Log layers">
              <For each={layers}>
                {(layer) => (
                  <button
                    class="dt-chip"
                    aria-pressed={activeLayers().has(layer) ? 'true' : 'false'}
                    onClick={() => toggleLayer(layer)}
                  >
                    {layer}
                  </button>
                )}
              </For>
            </div>
            <div class="dt-log-actions">
              <details class="dt-levels">
                <summary>
                  Levels{' '}
                  <span>
                    {activeLevels().size}/{levels.length}
                  </span>
                </summary>
                <div>
                  <For each={levels}>
                    {(level) => (
                      <label>
                        <input
                          type="checkbox"
                          checked={activeLevels().has(level)}
                          onChange={() => toggleLevel(level)}
                        />
                        {level}
                      </label>
                    )}
                  </For>
                </div>
              </details>
              <button class="dt-text-button" onClick={download} disabled={!tools.entries().length}>
                Save
              </button>
              <button
                class="dt-text-button"
                onClick={tools.clear}
                disabled={!tools.entries().length}
              >
                Clear
              </button>
            </div>
          </div>
          <div
            class="dt-viewport"
            ref={(element) => {
              viewport = element;
            }}
            tabindex={0}
            role="region"
            aria-label="Connection log"
            onScroll={() => {
              if (viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop > 48)
                setFollow(false);
            }}
          >
            <Show
              when={entries().length}
              fallback={
                <div class="dt-empty">
                  {tools.entries().length ? 'No matching entries' : 'No connection activity'}
                </div>
              }
            >
              <div class="dt-lines">
                <For each={entries()}>{(entry) => <LogRow entry={entry} />}</For>
              </div>
            </Show>
          </div>
          <ToolActivity session={share} />
          <div class="dt-log-footer">
            <span>
              {entries().length.toLocaleString()} / {tools.entries().length.toLocaleString()}{' '}
              entries
              <Show when={tools.discarded()}>
                {' '}
                · {tools.discarded().toLocaleString()} discarded
              </Show>
            </span>
            <label>
              <input
                type="checkbox"
                checked={follow()}
                onChange={(event) => setFollow(event.currentTarget.checked)}
              />
              Follow
            </label>
          </div>
          <form class="dt-composer" onSubmit={send}>
            <span class="dt-prompt" aria-hidden="true">
              ›
            </span>
            <input
              ref={(element) => {
                input = element;
              }}
              aria-label="Console command"
              placeholder="Console command"
              value={draft()}
              maxlength={
                4096 - (ending() === LineEnding.CrLf ? 2 : ending() === LineEnding.Lf ? 1 : 0)
              }
              onInput={(event) => setDraft(event.currentTarget.value)}
              onKeyDown={recall}
              disabled={
                tools.connection() !== ConnectionState.Connected ||
                tools.consoleState() === ConsoleState.Unknown
              }
              enterkeyhint="send"
              spellcheck={false}
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
            />
            <button class="pill dt-primary" type="submit" disabled={!canSend() || !draft().trim()}>
              Send ↵
            </button>
          </form>
          <div class="dt-composer-footer">
            <label>
              Line ending{' '}
              <select
                aria-label="Line ending"
                value={ending()}
                onChange={(event) => setEnding(event.currentTarget.value as LineEnding)}
              >
                <For each={Object.values(LineEnding)}>
                  {(value) => <option value={value}>{value}</option>}
                </For>
              </select>
            </label>
            <Show when={tools.consoleState() === ConsoleState.Polling}>
              <button class="dt-text-button" onClick={tools.stopPolling}>
                Stop waiting
              </button>
            </Show>
            <Show
              when={
                tools.connection() === ConnectionState.Connected &&
                (tools.consoleState() === ConsoleState.Pending ||
                  tools.consoleState() === ConsoleState.Unknown)
              }
            >
              <button
                class="dt-text-button"
                onClick={() => {
                  void tools.readResponse();
                }}
              >
                Read response
              </button>
            </Show>
            <Show when={tools.consoleState() === ConsoleState.Unknown}>
              <span class="dt-warning">Completion unknown</span>
            </Show>
            <span class="dt-device">{tools.camera()?.name ?? 'No camera'}</span>
          </div>
        </div>
      </main>
    </div>
  );
}
