import type { InspectorRead } from './inspection';
import { initialize, usb, type Device, type Session } from 'fujimod';
import { exportUsbDevice, type SharedDevice, type BrowserUsbDevice } from './usb-sharing';
import { Direction, Layer, Level, describeCommand, type NewEntry } from './log';
export enum CameraTarget {
  Usb = 'usb',
  Remote = 'remote',
}
export enum Progress {
  Complete,
  Pending,
  Unknown,
}
export interface ConsoleOutput {
  progress: Progress;
  status: number;
  output: Uint8Array;
  unknown?: number;
}
export interface InterfaceChoice {
  id: number;
  label: string;
}
export interface CameraChoice {
  firmware?: { id: string; model: string; version: string; sha256: string };
  name: string;
  interfaces: InterfaceChoice[];
}
export type DebugModeSession = Pick<Session, 'armResetGuard' | 'enterDebugMode' | 'leaveDebugMode'>;
export interface CameraLink {
  debugSession?: DebugModeSession;
  inspect?: InspectorRead;
  submit(data: Uint8Array): Promise<void>;
  poll(): Promise<ConsoleOutput>;
  usable(): boolean;
  abandon(): void;
  close(): Promise<void>;
}
export interface CameraDiagnostics extends Record<string, unknown> {
  checkpoint?: ArrayBuffer;
  snapshot: Record<string, unknown>;
}
export interface CameraBackend {
  exportUsb?(): Promise<SharedDevice>;
  canShare?(): boolean;
  errors?(listener: (error: Error) => void): () => void;
  diagnostics?(): Promise<CameraDiagnostics>;
  initialize(): Promise<void>;
  pick(target?: CameraTarget): Promise<CameraChoice | undefined>;
  open(id: number): Promise<CameraLink>;
  drain(): NewEntry[];
  onDisconnect(callback: () => void): () => void;
  cancelPending?(): void;
  dispose(): void;
}
/** Give Inspector only the session's read operations. */
export function sessionInspectorRead(
  session: Pick<
    Session,
    'deviceInfo' | 'configurationGeometry' | 'readConfiguration' | 'readMemory'
  >,
): InspectorRead {
  return async (request) => {
    switch (request.kind) {
      case 'identity': {
        const reply = await session.deviceInfo(usb.InfoSelector[request.selector]);
        try {
          return { text: reply.text, data: reply.raw };
        } finally {
          reply.free();
        }
      }
      case 'geometry': {
        const reply = await session.configurationGeometry(usb.ConfigurationView.Ram);
        try {
          return { capacity: reply.capacity, pageBytes: reply.pageBytes, data: reply.raw };
        } finally {
          reply.free();
        }
      }
      case 'configuration':
        return {
          data: await session.readConfiguration(
            usb.ConfigurationView.Ram,
            0,
            request.offset,
            request.length,
          ),
        };
      case 'memory':
        return { data: await session.readMemory(request.address, request.length) };
    }
  };
}
const CONSOLE_BYTES = 4096;
abstract class SessionCamera implements CameraBackend {
  abstract pick(): Promise<CameraChoice | undefined>;
  protected device?: Device;
  protected activeLink?: CameraLink;
  protected interfaces: Awaited<ReturnType<Device['interfaces']>> = [];
  private trace?: InstanceType<typeof usb.tracing.Trace>;
  async initialize(): Promise<void> {
    await initialize();
  }
  async open(id: number): Promise<CameraLink> {
    if (!this.device || !this.interfaces[id]) throw Error('Choose a USB interface.');
    this.trace?.free();
    this.trace = new usb.tracing.Trace(4096, 8 * 1024 * 1024);
    const options = new usb.TransferOptions(5000, 65536);
    // Host budgets; workflows validate the selected firmware's camera table
    // and instruction bytes before making any camera changes.
    const limits = new usb.FirmwareLimits(16384, 16384, 256, CONSOLE_BYTES);
    options.setTrace(this.trace);
    let session: Session;
    try {
      session = await usb.Session.open(this.device, this.interfaces[id], limits, options);
    } finally {
      options.free();
      limits.free();
    }
    let live = true;
    const link: CameraLink = {
      debugSession: session,
      inspect: sessionInspectorRead(session),
      async submit(data) {
        await session.submitConsole(data);
      },
      async poll() {
        const reply = await session.pollConsole(CONSOLE_BYTES);
        try {
          return {
            progress:
              reply.progress === usb.ConsoleProgress.Complete
                ? Progress.Complete
                : reply.progress === usb.ConsoleProgress.Pending
                  ? Progress.Pending
                  : Progress.Unknown,
            status: reply.status,
            output: reply.output,
            unknown: reply.unknownProgress,
          };
        } finally {
          reply.free();
        }
      },
      usable: () => live && session.state === usb.SessionState.Ready,
      async close() {
        if (!live) return;
        live = false;
        try {
          await session.close();
        } finally {
          session.free();
        }
      },
      abandon() {
        if (live) {
          live = false;
          session.abandon();
          session.free();
        }
      },
    };
    this.activeLink = link;
    return link;
  }
  drain(): NewEntry[] {
    if (!this.trace) return [];
    const batch = this.trace.drain();
    const records: { sequence: bigint; entry: NewEntry }[] = [];
    try {
      for (const event of batch.events) {
        try {
          const submitted = event.phase === usb.tracing.Phase.Submitted;
          const failed =
            event.phase === usb.tracing.Phase.Failed || event.phase === usb.tracing.Phase.Canceled;
          const direction = event.stage === usb.tracing.Stage.BulkIn ? Direction.In : Direction.Out;
          const stage =
            event.stage === usb.tracing.Stage.Control
              ? `Control ${event.requestKind === undefined ? '' : usb.tracing.RequestKind[event.requestKind]}`
              : usb.tracing.Stage[event.stage];
          records.push({
            sequence: event.sequence,
            entry: {
              time: event.timestampMs,
              layer: Layer.Usb,
              level: failed ? Level.Error : Level.Trace,
              direction,
              message: `${stage} ${submitted ? 'submitted' : event.phase === usb.tracing.Phase.Completed ? 'completed' : event.phase === usb.tracing.Phase.Canceled ? 'canceled' : 'failed'}${event.endpoint === undefined ? '' : ` · ep 0x${event.endpoint.toString(16).padStart(2, '0')}`}`,
              data: event.data,
              detail:
                event.failureMessage ??
                (event.phase === usb.tracing.Phase.Canceled
                  ? 'Host wait canceled; camera execution may continue.'
                  : `${event.requested ?? event.transferred ?? 0} bytes`),
              correlation: `USB ${event.transfer}`,
            },
          });
        } finally {
          event.free();
        }
      }
      for (const event of batch.protocolEvents) {
        try {
          const out = event.phase === usb.tracing.Phase.Submitted;
          const failed =
            event.phase === usb.tracing.Phase.Failed || event.phase === usb.tracing.Phase.Canceled;
          const data = event.data;
          const layer = event.layer === usb.tracing.Layer.App80 ? Layer.App80 : Layer.Ff80;
          records.push({
            sequence: event.sequence,
            entry: {
              time: event.timestampMs,
              layer,
              level: failed ? Level.Error : Level.Debug,
              direction: out ? Direction.Out : Direction.In,
              message: `${layer === Layer.App80 ? `Command 0x${event.command.toString(16)}` : describeCommand(data, event.command)}${event.phase === usb.tracing.Phase.Canceled ? ' · canceled' : failed ? ' · failed' : ''}`,
              data,
              detail: event.failureMessage,
              correlation: `${layer} ${event.exchange}`,
            },
          });
        } finally {
          event.free();
        }
      }
      const entries = records
        .sort((a, b) => (a.sequence < b.sequence ? -1 : a.sequence > b.sequence ? 1 : 0))
        .map((record) => record.entry);
      if (batch.droppedEvents)
        entries.push({
          layer: Layer.Connection,
          level: Level.Warning,
          direction: Direction.Host,
          message: `Recorder discarded ${batch.droppedEvents} entries (${batch.droppedBytes} bytes)`,
        });
      return entries;
    } finally {
      batch.free();
    }
  }
  onDisconnect(_callback: () => void): () => void {
    return () => {};
  }
  protected releaseDevice(): void {
    for (const selection of this.interfaces) selection.free();
    this.interfaces = [];
    this.device?.free();
    this.device = undefined;
  }
  dispose(): void {
    this.releaseDevice();
    this.trace?.free();
    this.trace = undefined;
  }
}

export class WebUsbCamera extends SessionCamera {
  async pick(): Promise<CameraChoice | undefined> {
    this.releaseDevice();
    this.device = await usb.requestDevice(usb.DeviceFilter.Diagnostic);
    if (!this.device) return;
    this.interfaces = await this.device.interfaces();
    return {
      name:
        this.device.productName ||
        `Fujifilm ${this.device.productId.toString(16).padStart(4, '0')}`,
      interfaces: this.interfaces.map((selection, id) => ({
        id,
        label: `Configuration ${selection.configuration} · Interface ${selection.interface} · Alternate ${selection.alternate}`,
      })),
    };
  }
  async exportUsb(): Promise<SharedDevice> {
    if (!this.device || !this.activeLink?.usable()) throw Error('No connected camera');
    const usb = (navigator as Navigator & { usb?: { getDevices(): Promise<BrowserUsbDevice[]> } })
      .usb;
    const matches = ((await usb?.getDevices()) ?? []).filter(
      (raw) =>
        raw.vendorId === this.device!.vendorId &&
        raw.productId === this.device!.productId &&
        raw.serialNumber === this.device!.serialNumber,
    );
    if (matches.length !== 1) throw Error('Cannot uniquely select the connected USB device');
    await this.activeLink.close();
    this.activeLink = undefined;
    return exportUsbDevice(matches[0]);
  }
  onDisconnect(callback: () => void): () => void {
    const usbEvents = (navigator as Navigator & { usb?: EventTarget }).usb;
    const listener = (event: Event) => {
      const detached = (
        event as Event & { device?: { vendorId: number; productId: number; serialNumber?: string } }
      ).device;
      if (
        detached &&
        this.device &&
        detached.vendorId === this.device.vendorId &&
        detached.productId === this.device.productId &&
        detached.serialNumber === this.device.serialNumber
      )
        callback();
    };
    usbEvents?.addEventListener('disconnect', listener);
    return () => usbEvents?.removeEventListener('disconnect', listener);
  }
}

export class RemoteUsbCamera extends SessionCamera {
  async pick(): Promise<CameraChoice | undefined> {
    await this.initialize();
    this.releaseDevice();
    const devices = await usb.discoverProxy(location.href, '', usb.DeviceFilter.Diagnostic);
    if (devices.length !== 1) {
      for (const device of devices) device.free();
      throw Error('Shared camera unavailable');
    }
    this.device = devices[0];
    this.interfaces = await this.device.interfaces();
    return {
      name: `Shared camera · ${this.device.productName || 'Fujifilm'}`,
      interfaces: this.interfaces.map((selection, id) => ({
        id,
        label: `Configuration ${selection.configuration} · Interface ${selection.interface} · Alternate ${selection.alternate}`,
      })),
    };
  }
}
