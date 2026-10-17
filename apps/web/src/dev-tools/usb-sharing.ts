import { Exporter } from '../../../../crates/usb-proxy/web/usb-exporter.js';
import type {
  SharedDevice,
  Descriptor,
  Operation,
  Completion,
} from '../../../../crates/usb-proxy/web/peer.js';
export type { SharedDevice, Descriptor, Operation, Completion };
export interface BrowserUsbDevice {
  vendorId: number;
  productId: number;
  serialNumber?: string;
  configuration?: { interfaces: { claimed: boolean }[] } | null;
}
/** A selected, connected USB device. The existing Rust session must be dropped
 * before this factory runs; no firmware state is inferred or rewritten here. */
export async function exportUsbDevice(device: BrowserUsbDevice): Promise<SharedDevice> {
  const until = performance.now() + 2000;
  while (device.configuration?.interfaces.some((iface) => iface.claimed)) {
    if (performance.now() >= until) throw Error('USB interface is still in use');
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  const exporter = new Exporter({ getDevices: async () => [device] });
  const [descriptor] = (await exporter.inventory()) as Descriptor[];
  let serial = 0;
  const pending = new Map<number, (completion: Completion) => void>();
  exporter.onreply = (frame: { id: number; completion: Completion }) => {
    pending.get(frame.id)?.(frame.completion);
    pending.delete(frame.id);
  };
  const execute = (operation: Operation, timeout: number): Promise<Completion> =>
    new Promise((resolve) => {
      const id = ++serial;
      pending.set(id, resolve);
      void exporter.request({ id, device: descriptor.id, operation, timeout_ms: timeout });
    });
  return {
    descriptor,
    execute,
    async close() {
      const completion = await execute({ op: 'close' }, 5000);
      if (completion.error) throw Error(completion.error);
    },
  };
}
