import type { InspectorRead } from './inspection';
import { RemoteTrafficLog } from './remote-traffic';
import type { ShareTrafficEvent } from '../../../../crates/usb-proxy/web/share-traffic.ts';
import { errorReportZip, type ReportSubmission } from '../engine/report';
import { createSignal } from 'solid-js';
import {
  CameraTarget,
  Progress,
  type CameraBackend,
  type CameraChoice,
  type CameraLink,
} from './camera';
import { Direction, Layer, Level, LogBuffer, type LogEntry, type NewEntry } from './log';
export enum ConnectionState {
  Loading,
  Disconnected,
  Choosing,
  Selecting,
  Opening,
  Connected,
  Sharing,
  Closing,
  Blocked,
  Unsupported,
}
export enum ConsoleState {
  Idle,
  Polling,
  Pending,
  Unknown,
}
export enum LineEnding {
  Lf = 'LF',
  CrLf = 'CRLF',
  None = 'None',
}
export function createDevTools(
  backend: CameraBackend,
  supported: boolean,
  onError: (error: unknown) => void = () => {},
) {
  const offlineState = supported ? ConnectionState.Disconnected : ConnectionState.Unsupported;
  const [connection, publishConnection] = createSignal(ConnectionState.Loading);
  const [consoleState, publishConsoleState] = createSignal(ConsoleState.Idle);
  // Application policy advances synchronously; Solid commits the displayed state
  // in a batch. Never use an uncommitted UI signal as an operation guard.
  let inspecting = false;
  let interfaceId: number | undefined;
  type LocalClaim = { release(): Promise<void>; abandon(): void };
  let claimLocal: (() => LocalClaim) | undefined;
  const [shareReady, setShareReady] = createSignal(false);
  let localClaim: LocalClaim | undefined;
  const [inspectionBusy, setInspectionBusy] = createSignal(false);
  let state = ConnectionState.Loading;
  let attempt = 0;
  let commandState = ConsoleState.Idle;
  const setConnection = (next: ConnectionState) => {
    state = next;
    publishConnection(next);
  };
  const setConsoleState = (next: ConsoleState) => {
    commandState = next;
    publishConsoleState(next);
  };
  const currentConnection = () => state;
  // Which source the current camera came from (physical USB or remote share).
  let connectedTarget: CameraTarget | undefined;
  const [camera, setCamera] = createSignal<CameraChoice>();
  const [entries, setEntries] = createSignal<LogEntry[]>([]);
  const [discarded, setDiscarded] = createSignal(0);
  const buffer = new LogBuffer();
  const remoteTraffic = new RemoteTrafficLog();
  let lastError: unknown;
  let link: CameraLink | undefined,
    disposed = false,
    stop = false;
  let decoder = new TextDecoder('utf-8');
  let timer: ReturnType<typeof setInterval> | undefined;
  let unlistenError: (() => void) | undefined;
  let unlisten: (() => void) | undefined;
  const log = (values: NewEntry[]) => {
    if (disposed || !values.length) return;
    buffer.append(values);
    setEntries(buffer.entries.slice());
    setDiscarded(buffer.discarded);
  };
  const message = (text: string, level = Level.Info) =>
    log([{ layer: Layer.Connection, level, direction: Direction.Host, message: text }]);
  const drain = () => log([...backend.drain(), ...remoteTraffic.drain()]);
  const fail = (error: unknown) => {
    if (lastError === error) return;
    lastError = error;
    drain();
    message(error instanceof Error ? error.message : String(error), Level.Error);
    if (link && !link.usable()) {
      setConnection(ConnectionState.Blocked);
      setConsoleState(ConsoleState.Unknown);
    }
    onError(error);
  };
  function disconnect(abandon = false) {
    const closingAttempt = ++attempt;
    stop = true;
    const old = link;
    const shared = localClaim !== undefined;
    localClaim?.abandon();
    localClaim = undefined;
    claimLocal = undefined;
    link = undefined;
    const clean =
      !abandon && !shared && !inspecting && old?.usable() && commandState === ConsoleState.Idle;
    setConsoleState(ConsoleState.Idle);
    const finish = () => {
      if (disposed || closingAttempt !== attempt) return;
      backend.cancelPending?.();
      drain();
      log(remoteTraffic.end());
      setConnection(offlineState);
      if (old) message('Disconnected');
    };
    if (clean && old) {
      setConnection(ConnectionState.Closing);
      return old
        .close()
        .catch((error: unknown) => {
          if (!disposed)
            message(error instanceof Error ? error.message : String(error), Level.Error);
        })
        .finally(finish);
    } else {
      old?.abandon();
      finish();
      return Promise.resolve();
    }
  }
  async function open(id: number) {
    if (currentConnection() !== ConnectionState.Selecting) return;
    setConnection(ConnectionState.Opening);
    const currentAttempt = attempt;
    try {
      const opened = await backend.open(id);
      if (
        disposed ||
        currentAttempt !== attempt ||
        currentConnection() !== ConnectionState.Opening
      ) {
        opened.abandon();
        return;
      }
      interfaceId = id;
      link = opened;
      drain();
      setConnection(ConnectionState.Connected);
      message(`Connected · ${camera()?.name ?? 'Fujifilm'}`);
    } catch (error) {
      if (!disposed && currentAttempt === attempt) {
        fail(error);
        setConnection(offlineState);
      }
    }
  }
  async function connect(target = CameraTarget.Usb) {
    if (target === CameraTarget.Usb && !supported) return;
    connectedTarget = target;
    if (
      currentConnection() !== ConnectionState.Disconnected &&
      !(currentConnection() === ConnectionState.Unsupported && target !== CameraTarget.Usb)
    )
      return;
    setConnection(ConnectionState.Choosing);
    const currentAttempt = ++attempt;
    try {
      const choice = await backend.pick(target);
      if (
        disposed ||
        currentAttempt !== attempt ||
        currentConnection() !== ConnectionState.Choosing
      )
        return;
      if (!choice) {
        setConnection(offlineState);
        return;
      }
      setCamera(choice);
      setConnection(ConnectionState.Selecting);
      if (!choice.interfaces.length) {
        message('No bulk USB interface is available.', Level.Error);
        setConnection(offlineState);
      } else if (choice.interfaces.length === 1) await open(choice.interfaces[0].id);
    } catch (error) {
      if (!disposed && currentAttempt === attempt) {
        fail(error);
        setConnection(offlineState);
      }
    }
  }
  async function localSession(): Promise<CameraLink> {
    if (state === ConnectionState.Connected && link) return link;
    if (state !== ConnectionState.Sharing || !claimLocal || interfaceId === undefined)
      throw Error('No idle ff80 session can form commands.');
    if (link) return link;
    const claim = claimLocal();
    localClaim = claim;
    const mine = attempt;
    try {
      const opened = await backend.open(interfaceId);
      if (disposed || mine !== attempt || state !== ConnectionState.Sharing) {
        opened.abandon();
        throw Error('Camera disconnected');
      }
      link = opened;
      return opened;
    } catch (error) {
      claim.abandon();
      if (localClaim === claim) localClaim = undefined;
      if (!disposed && mine === attempt) setConnection(ConnectionState.Blocked);
      throw error;
    }
  }
  async function closeLocalSession(current: CameraLink) {
    if (!localClaim || link !== current) return;
    const claim = localClaim;
    try {
      if (!current.usable()) throw Error('Local session completion unknown; reconnect the camera.');
      await current.close();
      await claim.release();
    } catch (error) {
      current.abandon();
      claim.abandon();
      setConnection(ConnectionState.Blocked);
      fail(error);
      throw error;
    } finally {
      if (link === current) link = undefined;
      if (localClaim === claim) localClaim = undefined;
      drain();
    }
  }
  async function poll(current: CameraLink) {
    if (current !== link || commandState === ConsoleState.Polling) return;
    stop = false;
    setConsoleState(ConsoleState.Polling);
    const deadline = Date.now() + 10000;
    try {
      do {
        const reply = await current.poll();
        if (current !== link || disposed) return;
        drain();
        const text = decoder.decode(reply.output, { stream: reply.progress === Progress.Pending });
        if (reply.output.length)
          log([
            {
              layer: Layer.Console,
              level: Level.Info,
              direction: Direction.In,
              message: text || 'Console output',
              data: reply.output,
            },
          ]);
        if (reply.status) message(`Console status 0x${reply.status.toString(16)}`, Level.Warning);
        switch (reply.progress) {
          case Progress.Complete:
            await closeLocalSession(current);
            setConsoleState(ConsoleState.Idle);
            return;
          case Progress.Unknown:
            setConsoleState(ConsoleState.Unknown);
            message(
              `Unknown console progress ${reply.unknown ?? '?'}; command completion is unknown.`,
              Level.Error,
            );
            return;
          case Progress.Pending:
            break;
        }
        if (stop || Date.now() >= deadline) break;
        await new Promise((resolve) => setTimeout(resolve, 150));
      } while (current === link && !disposed);
      if (current === link && !disposed) {
        setConsoleState(ConsoleState.Pending);
        message('Console is still pending; read the response to continue.', Level.Warning);
      }
    } catch (error) {
      if (current === link && !disposed) {
        setConsoleState(ConsoleState.Unknown);
        fail(error);
      }
    }
  }
  async function send(text: string, ending: LineEnding): Promise<boolean> {
    let current: CameraLink | undefined;
    const sendingAttempt = attempt;
    if (
      ![ConnectionState.Connected, ConnectionState.Sharing].includes(state) ||
      commandState !== ConsoleState.Idle ||
      inspecting
    )
      return false;
    if (!text.trim() || /[^\x20-\x7e]/.test(text)) {
      message('Enter one ASCII console command.', Level.Error);
      return false;
    }
    const suffix = ending === LineEnding.Lf ? '\n' : ending === LineEnding.CrLf ? '\r\n' : '';
    const data = new TextEncoder().encode(text + suffix);
    if (data.length > 4096) {
      message('Console command exceeds 4096 bytes.', Level.Error);
      return false;
    }
    stop = false;
    decoder = new TextDecoder('utf-8');
    setConsoleState(ConsoleState.Polling);
    log([
      {
        layer: Layer.Console,
        level: Level.Info,
        direction: Direction.Out,
        message: text,
        data,
        detail: `Line ending: ${ending}`,
      },
    ]);
    try {
      current = state === ConnectionState.Connected ? link : await localSession();
      if (!current || current !== link || disposed) return false;
      await current.submit(data);
      if (current !== link || disposed) return true;
      drain();
      setConsoleState(ConsoleState.Pending);
      if (!stop) await poll(current);
      return true;
    } catch (error) {
      if (!disposed && sendingAttempt === attempt) {
        setConsoleState(current ? ConsoleState.Unknown : ConsoleState.Idle);
        fail(error);
      }
      return false;
    }
  }
  async function start() {
    try {
      await backend.initialize();
      if (disposed) return;
      setConnection(supported ? ConnectionState.Disconnected : ConnectionState.Unsupported);
      timer = setInterval(drain, 50);
      unlistenError = backend.errors?.(fail);
      unlisten = backend.onDisconnect(() => {
        disconnect(true);
        message('USB device removed', Level.Warning);
      });
    } catch (error) {
      fail(error);
      setConnection(ConnectionState.Unsupported);
    }
  }
  function dispose() {
    if (disposed) return;
    stop = true;
    link?.abandon();
    localClaim?.abandon();
    localClaim = undefined;
    link = undefined;
    if (timer) clearInterval(timer);
    unlisten?.();
    unlistenError?.();
    backend.dispose();
    remoteTraffic.end();
    disposed = true;
  }
  return {
    connection,
    consoleState,
    camera,
    inspectionBusy,
    localAvailable: () =>
      connection() === ConnectionState.Connected ||
      (connection() === ConnectionState.Sharing && shareReady()),
    async readInspection<T>(action: (read: InspectorRead) => Promise<T>): Promise<T> {
      let current: CameraLink | undefined;
      if (
        ![ConnectionState.Connected, ConnectionState.Sharing].includes(state) ||
        commandState !== ConsoleState.Idle ||
        inspecting
      )
        throw Error('No idle ff80 session can form inspection commands.');
      inspecting = true;
      setInspectionBusy(true);
      try {
        current = await localSession();
        if (!current.inspect || !current.usable())
          throw Error('No idle ff80 session can form inspection commands.');
        const session = current;
        const result = await action(async (request) => {
          if (link !== session || !session.usable())
            throw Error('Camera disconnected during inspection.');
          const label =
            request.kind === 'identity'
              ? `Identity · ${request.selector}`
              : request.kind === 'memory'
                ? `Memory · 0x${request.address.toString(16)} · ${request.length} bytes`
                : request.kind === 'configuration'
                  ? `Configuration · 0x${request.offset.toString(16)} · ${request.length} bytes`
                  : 'Configuration geometry';
          message(label);
          const reply = await session.inspect!(request);
          if (link !== session || !session.usable())
            throw Error('Camera disconnected during inspection.');
          if (
            (request.kind === 'memory' || request.kind === 'configuration') &&
            reply.data.length !== request.length
          )
            throw Error(
              `Read length mismatch: expected ${request.length}, received ${reply.data.length}.`,
            );
          drain();
          log([
            {
              layer: Layer.Connection,
              level: Level.Info,
              direction: Direction.In,
              message: `${label}${reply.text === undefined ? '' : ` · ${reply.text}`}`,
              data: reply.data,
              detail: JSON.stringify({ ...reply, data: undefined }),
            },
          ]);
          return reply;
        });
        await closeLocalSession(current);
        return result;
      } catch (error) {
        fail(error);
        try {
          if (current) await closeLocalSession(current);
        } catch {
          // Cleanup already fenced and reported the broken session. Preserve
          // the operation's original failure for its status and diagnostic bundle.
        }
        throw error;
      } finally {
        inspecting = false;
        setInspectionBusy(false);
        drain();
      }
    },
    debugSession: () =>
      connection() === ConnectionState.Connected &&
      consoleState() === ConsoleState.Idle &&
      !inspectionBusy() &&
      connectedTarget === CameraTarget.Usb
        ? link?.debugSession
        : undefined,
    target: () => connectedTarget,
    canShare: () => Boolean(backend.exportUsb) && (backend.canShare?.() ?? true),
    connectionMessage: message,
    shareTraffic: (event: ShareTrafficEvent) => {
      if (!disposed) remoteTraffic.record(event);
    },
    shareTrafficEnded: (controller: string) => log(remoteTraffic.end(controller)),
    async shareUsb() {
      if (
        state !== ConnectionState.Connected ||
        commandState !== ConsoleState.Idle ||
        inspecting ||
        !backend.exportUsb
      )
        throw Error('Camera is not available for sharing');
      setShareReady(false);
      setConnection(ConnectionState.Sharing);
      const currentAttempt = attempt;
      try {
        const device = await backend.exportUsb();
        if (disposed || currentAttempt !== attempt) {
          await device.close();
          throw Error('Camera disconnected');
        }
        link = undefined;
        drain();
        log(remoteTraffic.begin(device.descriptor));
        message('Camera sharing started');
        return device;
      } catch (error) {
        if (!disposed && currentAttempt === attempt) {
          setConnection(ConnectionState.Blocked);
          fail(error);
        }
        throw error;
      }
    },
    sharingReady(claim: () => LocalClaim) {
      claimLocal = claim;
      setShareReady(true);
    },
    sharingEnded() {
      setShareReady(false);
      claimLocal = undefined;
      log(remoteTraffic.end());
      if (state === ConnectionState.Sharing) {
        setConnection(ConnectionState.Blocked);
        message('Camera sharing ended');
      }
    },
    entries,
    discarded,
    reportContext() {
      return {
        connection: ConnectionState[state],
        console: ConsoleState[commandState],
        commands: buffer.entries
          .filter((entry) => entry.layer === Layer.Console && entry.direction === Direction.Out)
          .slice(-8)
          .map((entry) => entry.message),
        logTail: buffer.entries.slice(-64).map((entry) => ({
          time: entry.time,
          layer: entry.layer,
          level: entry.level,
          direction: entry.direction,
          message: entry.message.slice(0, 2048),
        })),
      };
    },
    async errorReport(
      this: void,
      submission: Promise<ReportSubmission> = Promise.resolve({ description: '' }),
    ) {
      drain();
      const logs = buffer.entries.slice(),
        choice = camera();
      let diagnostics;
      try {
        diagnostics = (await backend.diagnostics?.()) ?? {
          snapshot: { unavailable: 'No emulator diagnostics provider.' },
        };
      } catch (error) {
        diagnostics = { snapshot: { unavailable: String(error) } };
      }
      const { checkpoint, ...detail } = diagnostics;
      const attachments: [string, Uint8Array<ArrayBuffer>][] = logs
        .filter((entry) => entry.data?.length)
        .map((entry) => [`usb/${entry.id}.bin`, entry.data!.slice()]);
      return errorReportZip(
        {
          schema: 'fujimod-error-report-v1',
          createdAt: new Date().toISOString(),
          ...detail,
          source: 'dev-tools',
          identity: { ...(detail.identity as object), ...choice?.firmware },
          connection: ConnectionState[state],
          console: ConsoleState[commandState],
          error:
            lastError instanceof Error
              ? { name: lastError.name, message: lastError.message, stack: lastError.stack }
              : lastError,
          logs: logs.map((entry) => ({
            ...entry,
            data: undefined,
            attachment: entry.data?.length ? `usb/${entry.id}.bin` : undefined,
          })),
          browser: { userAgent: navigator.userAgent },
          ...(await submission),
        },
        checkpoint,
        attachments,
      );
    },
    start,
    connect,
    open,
    disconnect,
    send,
    readResponse: () =>
      link &&
      link.usable() &&
      (commandState === ConsoleState.Pending || commandState === ConsoleState.Unknown)
        ? poll(link)
        : Promise.resolve(),
    stopPolling: () => {
      stop = true;
    },
    clear: () => {
      backend.drain();
      remoteTraffic.clear();
      buffer.clear();
      setEntries([]);
      setDiscarded(0);
    },
    dispose,
  };
}
export type DevToolsController = ReturnType<typeof createDevTools>;
