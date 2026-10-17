import { For, Show, createEffect, createMemo, createSignal, onSettled, untrack } from 'solid-js';
import { CameraTarget, type CameraBackend } from './camera';
import { ConnectionState, ConsoleState, LineEnding, createDevTools } from './controller';
import { Direction, Layer, Level, exportLog, hex, type LogEntry } from './log';
import { scenarios } from './scenarios';
import { LocalReports } from '../ui/local-reports';
import { Feedback } from '../ui/Feedback';
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
export function DevTools(props: { backend?: CameraBackend; supported?: boolean }) {
  const tools = createDevTools(
    untrack(() => props.backend) ?? new CameraSources(),
    untrack(() => props.supported) ?? (isSecureContext && 'usb' in navigator),
    (error) => reports.crash(error),
  );
  const context = () => ({
    event:
      tools.connection() === ConnectionState.Blocked ? ('crash' as const) : ('feedback' as const),
    camera: tools.camera()?.firmware,
    diagnostics: tools.reportContext(),
    modBuild: 'Stock firmware (no mods)',
    simulatorBuild: __FUJIMOD_SIMULATOR_BUILD__,
  });
  const reports = new LocalReports('dev-tools', context, tools.errorReport);
  const [activeLayers, setActiveLayers] = createSignal<ReadonlySet<Layer>>(
    new Set([Layer.Console, Layer.App80, Layer.Ff80]),
  );
  const [activeLevels, setActiveLevels] = createSignal<ReadonlySet<Level>>(new Set(levels));
  const [target, setTarget] = createSignal(
    new URLSearchParams(location.search).get('target') === CameraTarget.Emulator
      ? CameraTarget.Emulator
      : CameraTarget.Usb,
  );
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
    void tools.start().then(async () => {
      if (
        import.meta.env.DEV &&
        new URLSearchParams(location.search).get('target') === CameraTarget.Emulator &&
        new URLSearchParams(location.search).get('connect') === '1'
      ) {
        await tools.connect(CameraTarget.Emulator);
        const scenario = scenarios[new URLSearchParams(location.search).get('scenario') ?? ''];
        if (scenario && tools.camera()?.firmware?.id === scenario.firmware) {
          for (const command of scenario.commands) {
            if (!(await tools.send(command, LineEnding.Lf))) break;
          }
        }
      }
    });
    return () => {
      unlisten();
      tools.dispose();
    };
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
        <Show when={target() === CameraTarget.Emulator}>
          <Feedback context={context} capture={tools.errorReport} local={reports} />
        </Show>
        <div class="dt-connection">
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
            <option value={CameraTarget.Emulator}>Simulator</option>
          </select>
          <span class="dt-status" role="status">
            <i class={tools.connection() === ConnectionState.Connected ? 'online' : ''} />
            {tools.connection() === ConnectionState.Unsupported &&
            target() === CameraTarget.Emulator
              ? 'Disconnected'
              : tools.connection() === ConnectionState.Choosing &&
                  target() === CameraTarget.Emulator
                ? 'Starting simulator'
                : connectionNames[tools.connection()]}
          </span>
          <Show
            when={
              tools.connection() === ConnectionState.Disconnected ||
              (tools.connection() === ConnectionState.Unsupported &&
                target() === CameraTarget.Emulator)
            }
            fallback={
              <Show
                when={
                  tools.connection() === ConnectionState.Connected ||
                  tools.connection() === ConnectionState.Blocked ||
                  tools.connection() === ConnectionState.Choosing ||
                  tools.connection() === ConnectionState.Opening
                }
              >
                <button class="pill" onClick={tools.disconnect}>
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
                void tools.connect(target());
              }}
            >
              {target() === CameraTarget.Emulator ? 'Connect simulator' : 'Connect camera'}
            </button>
          </Show>
        </div>
      </header>
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
            <button class="pill" type="button" onClick={tools.disconnect}>
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
