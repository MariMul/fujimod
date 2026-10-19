import type { UsbDevice } from '../../../../crates/usb-proxy/web/webusb.ts';
import { Exporter } from '../../../../crates/usb-proxy/web/usb-exporter.ts';
import type {
  SharedDevice,
  Descriptor,
  Operation,
  Completion,
} from '../../../../crates/usb-proxy/web/peer.ts';
export type { SharedDevice, Descriptor, Operation, Completion };
export interface BrowserUsbDevice extends UsbDevice {}
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
  const execute = (operation: Operation, timeout: number): Promise<Completion> =>
    exporter.run(descriptor.id, operation, timeout);
  return {
    descriptor,
    execute,
    async close() {
      const completion = await execute({ op: 'close' }, 5000);
      if (completion.error) throw Error(completion.error);
    },
  };
}
