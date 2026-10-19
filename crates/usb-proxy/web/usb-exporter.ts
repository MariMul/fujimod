import type { Operation, Completion, Descriptor } from './types.ts';
import type { Usb, UsbDevice } from './webusb.ts';
// Browser USB exporter. No camera commands, memory maps or firmware policy.
export const MAX_TRANSFER = 8 * 1024 * 1024;
const payload = (data: unknown) => {
  if (!(data instanceof Uint8Array)) throw Error('Expected binary USB payload');
  length(data.length);
  return data;
};
const length = (value: unknown): number => {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value > MAX_TRANSFER)
    throw Error('Invalid transfer length');
  return value;
};
export class Exporter {
  usb: Usb;
  changed: () => void;
  devices: Map<string, UsbDevice>;
  ids: WeakMap<UsbDevice, string>;
  sequence: number;
  claims: Map<string, number>;
  queues: Map<string, Promise<void>>;
  fenced: Set<string>;
  stopped: boolean;
  constructor(usb: Usb, changed = () => {}) {
    this.usb = usb;
    this.changed = changed;
    this.devices = new Map();
    this.ids = new WeakMap();
    this.sequence = 0;
    this.claims = new Map();
    this.queues = new Map();
    this.fenced = new Set();
    this.stopped = false;
  }
  async inventory(): Promise<Descriptor[]> {
    const devices = await this.usb.getDevices();
    this.devices.clear();
    return devices.map((device) => {
      let id = this.ids.get(device);
      if (!id) {
        id = String(++this.sequence);
        this.ids.set(device, id);
      }
      this.devices.set(id, device);
      return {
        id,
        vendor_id: device.vendorId,
        product_id: device.productId,
        manufacturer: device.manufacturerName || null,
        product: device.productName || null,
        serial: device.serialNumber || null,
        interfaces: device.configurations.flatMap((config) =>
          config.interfaces.flatMap((iface) =>
            iface.alternates.map((alternate) => ({
              configuration: config.configurationValue,
              interface: iface.interfaceNumber,
              alternate: alternate.alternateSetting,
              endpoints: alternate.endpoints.map((e) => ({
                address: e.endpointNumber | (e.direction === 'in' ? 128 : 0),
                kind: e.type,
                packet_size: e.packetSize,
              })),
            })),
          ),
        ),
      };
    });
  }
  async execute(id: string, operation: Operation): Promise<Completion> {
    const device = this.devices.get(id);
    if (!device) throw Error('Device disconnected');
    const op = operation.op;
    if (this.stopped && op !== 'close') throw Error('Exporter stopped');
    if (this.fenced.has(id) && op !== 'close') throw Error('Device fenced; reconnect exporter');
    if (op === 'open') {
      const configuration = length(operation.configuration),
        ifaceNumber = length(operation.interface),
        alternate = length(operation.alternate);
      if (this.claims.has(id)) throw Error('Device already claimed');
      await device.open();
      try {
        if (device.configuration?.configurationValue !== configuration)
          await device.selectConfiguration(configuration);
        await device.claimInterface(ifaceNumber);
        // Record the claim before alternate selection so failure can release it.
        this.claims.set(id, ifaceNumber);
        const selected = device.configuration?.interfaces.find(
          (iface) => iface.interfaceNumber === ifaceNumber,
        );
        if (!selected) throw Error('Interface unavailable');
        if (selected.alternate?.alternateSetting !== alternate)
          await device.selectAlternateInterface(ifaceNumber, alternate);
      } catch (error) {
        await device.close();
        this.claims.delete(id);
        throw error;
      }
      this.changed();
      return {};
    }
    if (op === 'close') {
      // WebUSB.close only releases host resources, never a camera protocol close.
      await device.close();
      this.claims.delete(id);
      this.changed();
      return {};
    }
    if (!this.claims.has(id)) throw Error('Device is not claimed');
    if (['transfer_in', 'transfer_out', 'clear_halt'].includes(op)) {
      const endpoint = operation.endpoint;
      if (
        typeof endpoint !== 'number' ||
        !Number.isInteger(endpoint) ||
        endpoint < 1 ||
        endpoint > 255 ||
        endpoint & 112 ||
        !(endpoint & 15) ||
        (op === 'transfer_in' && !(endpoint & 128)) ||
        (op === 'transfer_out' && endpoint & 128)
      )
        throw Error('Invalid endpoint');
      const iface = device.configuration?.interfaces.find(
        (i) => i.interfaceNumber === this.claims.get(id),
      );
      if (!iface) throw Error('Interface unavailable');
      const alternate =
        iface.alternate ||
        iface.alternates.find((a) => a.alternateSetting === iface.alternateSetting);
      if (
        !alternate?.endpoints.some(
          (e) =>
            e.endpointNumber === (endpoint & 15) &&
            e.direction === (endpoint & 128 ? 'in' : 'out') &&
            ['bulk', 'interrupt'].includes(e.type),
        )
      )
        throw Error('Endpoint unavailable');
    }
    let result;
    const setup = operation.setup && {
      requestType: operation.setup.request_type,
      recipient: operation.setup.recipient,
      request: operation.setup.request,
      value: operation.setup.value,
      index: operation.setup.index,
    };
    if (op.startsWith('control_') && !setup) throw Error('Missing control setup');
    if (op === 'control_in')
      result = await device.controlTransferIn(setup!, length(operation.length));
    else if (op === 'control_out')
      result = await device.controlTransferOut(setup!, payload(operation.data));
    else if (op === 'transfer_in')
      result = await device.transferIn(length(operation.endpoint) & 15, length(operation.length));
    else if (op === 'transfer_out')
      result = await device.transferOut(length(operation.endpoint) & 15, payload(operation.data));
    else if (op === 'clear_halt') {
      await device.clearHalt(
        length(operation.endpoint) & 128 ? 'in' : 'out',
        length(operation.endpoint) & 15,
      );
      return {};
    } else throw Error('Unsupported USB operation');
    const data = result.data
      ? new Uint8Array(result.data.buffer, result.data.byteOffset, result.data.byteLength)
      : new Uint8Array();
    return {
      data,
      transferred: result.bytesWritten ?? data.length,
      error: result.status === 'ok' ? null : result.status,
    };
  }
  /** Run one operation on device `id` after its earlier ones. A failure or
   * expired deadline fences the device; the physical transfer stays queued. */
  run(id: string, operation: Operation, timeout_ms: number): Promise<Completion> {
    // Network echoes bypass USB queues, including outstanding physical work.
    if (operation.op === 'probe') return Promise.resolve({});
    const previous = this.queues.get(id) || Promise.resolve();
    let reported!: (completion: Completion) => void;
    const reply = new Promise<Completion>((resolve) => (reported = resolve));
    const task = previous.then(async () => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const running = this.execute(id, operation);
      const timeout = new Promise<never>((_, reject) => {
        timer = setTimeout(() => {
          this.fenced.add(id);
          reject(Error('USB deadline expired; completion unknown'));
        }, timeout_ms);
      });
      reported(
        await Promise.race([running, timeout])
          .catch((error) => {
            this.fenced.add(id);
            return { error: error.message || String(error) };
          })
          .finally(() => clearTimeout(timer)),
      );
      // Keep the physical operation in the queue even after reporting timeout.
      await running.catch(() => {});
    });
    this.queues.set(
      id,
      task.catch(() => {}),
    );
    return reply;
  }
  async stop() {
    this.stopped = true;
    await Promise.all(this.queues.values());
    await Promise.all(
      [...this.devices.entries()]
        .filter(([id]) => this.claims.has(id))
        .map(async ([id, device]) => {
          try {
            await device.close();
            this.claims.delete(id);
          } catch {
            /* retain uncertainty */
          }
        }),
    );
  }
}
