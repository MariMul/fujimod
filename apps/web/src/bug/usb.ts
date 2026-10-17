import { initialize, usb, type Device, type Session } from 'fujimod';
import {
  Progress,
  type CameraBackend,
  type CameraChoice,
  type CameraLink,
} from '../dev-tools/camera';
import { retrieveReport } from './transfer';
import type { ReportLink } from './report-link';
export interface ReportConnection {
  initialize(): Promise<void>;
  choose(): Promise<CameraChoice | undefined>;
  read(id: number, link: ReportLink, progress: (bytes: number) => void): Promise<Blob>;
  devTools(): CameraBackend;
  onDisconnect?(callback: () => void): () => void;
  dispose(): void;
}
/** Owns one selected session; handing it to Dev Tools never chooses another device. */
export class ReportUsb implements ReportConnection {
  private device?: Device;
  private interfaces: Awaited<ReturnType<Device['interfaces']>> = [];
  private session?: Session;
  private choice?: CameraChoice;
  async initialize() {
    await initialize();
  }
  async choose() {
    this.dispose();
    this.device = await usb.requestDevice(usb.DeviceFilter.Diagnostic);
    if (!this.device) return;
    this.interfaces = await this.device.interfaces();
    this.choice = {
      name: this.device.productName || 'Fujifilm camera',
      interfaces: this.interfaces.map((item, id) => ({
        id,
        label: `Interface ${item.interface} · Alternate ${item.alternate}`,
      })),
    };
    return this.choice;
  }
  async read(id: number, link: ReportLink, progress: (bytes: number) => void) {
    if (!this.device || !this.interfaces[id]) throw Error('Choose a camera interface.');
    const limits = new usb.FirmwareLimits(16384, 32, 1, 4096),
      options = new usb.TransferOptions(5000, 65536);
    try {
      this.session = await usb.Session.open(this.device, this.interfaces[id], limits, options);
    } finally {
      limits.free();
      options.free();
    }
    const session = this.session;
    try {
      return await retrieveReport(
        {
          read: (address, count) => session.readMemory(address, count),
          write: (address, bytes) => session.writeMemory(address, bytes),
        },
        link,
        progress,
      );
    } catch (error) {
      // No close command or mode change after an unknown/wrong camera reply.
      session.abandon();
      session.free();
      this.session = undefined;
      throw error;
    }
  }
  devTools(): CameraBackend {
    const session = this.session,
      choice = this.choice,
      dispose = () => this.dispose();
    if (!session || !choice) throw Error('The camera is disconnected.');
    let live = true;
    const link: CameraLink = {
      async submit(data) {
        await session.submitConsole(data);
      },
      async poll() {
        const reply = await session.pollConsole(4096);
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
        if (live) {
          live = false;
          try {
            await session.close();
          } finally {
            dispose();
          }
        }
      },
      abandon() {
        if (live) {
          live = false;
          dispose();
        }
      },
    };
    return {
      initialize: async () => {},
      pick: async () => ({ ...choice, interfaces: [{ id: 0, label: 'Connected camera' }] }),
      open: async () => link,
      drain: () => [],
      onDisconnect: (callback) => this.onDisconnect(callback),
      dispose: () => dispose(),
    };
  }
  onDisconnect(callback: () => void) {
    const events = (navigator as Navigator & { usb?: EventTarget }).usb;
    const listener = (event: Event) => {
      const removed = (
        event as Event & { device?: { vendorId: number; productId: number; serialNumber?: string } }
      ).device;
      if (
        removed &&
        this.device &&
        removed.vendorId === this.device.vendorId &&
        removed.productId === this.device.productId &&
        removed.serialNumber === this.device.serialNumber
      )
        callback();
    };
    events?.addEventListener('disconnect', listener);
    return () => events?.removeEventListener('disconnect', listener);
  }
  dispose() {
    this.session?.abandon();
    this.session?.free();
    this.session = undefined;
    for (const item of this.interfaces) item.free();
    this.interfaces = [];
    this.device?.free();
    this.device = undefined;
  }
}
