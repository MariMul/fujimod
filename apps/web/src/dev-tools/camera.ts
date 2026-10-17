import { initialize, usb, type Device, type Session } from 'fujimod';
import { Direction, Layer, Level, describeCommand, type NewEntry } from './log';
export enum CameraTarget {
  Usb = 'usb',
  Emulator = 'simulator',
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
  name: string;
  interfaces: InterfaceChoice[];
}
export interface CameraLink {
  submit(data: Uint8Array): Promise<void>;
  poll(): Promise<ConsoleOutput>;
  usable(): boolean;
  abandon(): void;
}
export interface CameraBackend {
  initialize(): Promise<void>;
  pick(target?: CameraTarget): Promise<CameraChoice | undefined>;
  open(id: number): Promise<CameraLink>;
  drain(): NewEntry[];
  onDisconnect(callback: () => void): () => void;
  cancelPending?(): void;
  dispose(): void;
}
const CONSOLE_BYTES = 4096;
export class WebUsbCamera implements CameraBackend {
  private device?: Device;
  private interfaces: Awaited<ReturnType<Device['interfaces']>> = [];
  private trace?: InstanceType<typeof usb.tracing.Trace>;
  async initialize(): Promise<void> {
    await initialize();
  }
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
  async open(id: number): Promise<CameraLink> {
    if (!this.device || !this.interfaces[id]) throw Error('Choose a USB interface.');
    this.trace?.free();
    this.trace = new usb.tracing.Trace(4096, 8 * 1024 * 1024);
    const options = new usb.TransferOptions(5000, 65536);
    // Only console APIs are exposed by this adapter. These are host budgets,
    // not claims about a particular body's firmware or memory capabilities.
    const limits = new usb.FirmwareLimits(1, 1, 1, CONSOLE_BYTES);
    options.setTrace(this.trace);
    let session: Session;
    try {
      session = await usb.Session.open(this.device, this.interfaces[id], limits, options);
    } finally {
      options.free();
      limits.free();
    }
    let live = true;
    return {
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
      abandon() {
        if (live) {
          live = false;
          session.abandon();
          session.free();
        }
      },
    };
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
            event.phase === usb.tracing.Phase.Failed || event.phase === usb.tracing.Phase.Cancelled;
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
              message: `${stage} ${submitted ? 'submitted' : event.phase === usb.tracing.Phase.Completed ? 'completed' : event.phase === usb.tracing.Phase.Cancelled ? 'cancelled' : 'failed'}${event.endpoint === undefined ? '' : ` · ep 0x${event.endpoint.toString(16).padStart(2, '0')}`}`,
              data: event.data,
              detail:
                event.failureMessage ??
                (event.phase === usb.tracing.Phase.Cancelled
                  ? 'Host wait cancelled; camera execution may continue.'
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
            event.phase === usb.tracing.Phase.Failed || event.phase === usb.tracing.Phase.Cancelled;
          const data = event.data;
          const layer = event.layer === usb.tracing.Layer.App80 ? Layer.App80 : Layer.Ff80;
          records.push({
            sequence: event.sequence,
            entry: {
              time: event.timestampMs,
              layer,
              level: failed ? Level.Error : Level.Debug,
              direction: out ? Direction.Out : Direction.In,
              message: `${layer === Layer.App80 ? `Command 0x${event.command.toString(16)}` : describeCommand(data, event.command)}${event.phase === usb.tracing.Phase.Cancelled ? ' · cancelled' : failed ? ' · failed' : ''}`,
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
  private releaseDevice(): void {
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
