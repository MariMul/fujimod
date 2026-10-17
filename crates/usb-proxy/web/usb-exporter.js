// Browser USB exporter. No camera commands, memory maps or firmware policy.
export const MAX_TRANSFER = 8 * 1024 * 1024;
const encode = (data) => {
  let text = '';
  for (let i = 0; i < data.length; i += 32768)
    text += String.fromCharCode(...data.subarray(i, i + 32768));
  return btoa(text);
};
const decode = (text) => {
  if (typeof text !== 'string' || text.length > Math.ceil(MAX_TRANSFER / 3) * 4)
    throw Error('Transfer exceeds limit');
  const data = Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
  length(data.length);
  return data;
};
const length = (value) => {
  if (!Number.isInteger(value) || value < 0 || value > MAX_TRANSFER)
    throw Error('Invalid transfer length');
  return value;
};
export class Exporter {
  constructor(usb, changed = () => {}) {
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
  async inventory() {
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
  async execute(id, operation) {
    const device = this.devices.get(id);
    if (!device) throw Error('Device disconnected');
    const op = operation.op;
    if (this.stopped && op !== 'close') throw Error('Exporter stopped');
    if (this.fenced.has(id) && op !== 'close') throw Error('Device fenced; reconnect exporter');
    if (op === 'open') {
      if (this.claims.has(id)) throw Error('Device already claimed');
      await device.open();
      try {
        if (device.configuration?.configurationValue !== operation.configuration)
          await device.selectConfiguration(operation.configuration);
        await device.claimInterface(operation.interface);
        // Record the claim before alternate selection so failure can release it.
        this.claims.set(id, operation.interface);
        const selected = device.configuration.interfaces.find(
          (iface) => iface.interfaceNumber === operation.interface,
        );
        if (selected.alternate?.alternateSetting !== operation.alternate)
          await device.selectAlternateInterface(operation.interface, operation.alternate);
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
        !Number.isInteger(endpoint) ||
        endpoint < 1 ||
        endpoint > 255 ||
        endpoint & 112 ||
        !(endpoint & 15) ||
        (op === 'transfer_in' && !(endpoint & 128)) ||
        (op === 'transfer_out' && endpoint & 128)
      )
        throw Error('Invalid endpoint');
      const iface = device.configuration.interfaces.find(
        (i) => i.interfaceNumber === this.claims.get(id),
      );
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
    if (op === 'control_in')
      result = await device.controlTransferIn(setup, length(operation.length));
    else if (op === 'control_out')
      result = await device.controlTransferOut(setup, decode(operation.data));
    else if (op === 'transfer_in')
      result = await device.transferIn(operation.endpoint & 15, length(operation.length));
    else if (op === 'transfer_out')
      result = await device.transferOut(operation.endpoint & 15, decode(operation.data));
    else if (op === 'clear_halt') {
      await device.clearHalt(operation.endpoint & 128 ? 'in' : 'out', operation.endpoint & 15);
      return {};
    } else throw Error('Unsupported USB operation');
    const data = result.data
      ? new Uint8Array(result.data.buffer, result.data.byteOffset, result.data.byteLength)
      : new Uint8Array();
    return {
      data: encode(data),
      transferred: result.bytesWritten ?? data.length,
      error: result.status === 'ok' ? null : result.status,
    };
  }
  request(frame) {
    // Serialize per device, including late completions after a timed-out request.
    const previous = this.queues.get(frame.device) || Promise.resolve();
    const task = previous.then(async () => {
      let timer;
      const running = this.execute(frame.device, frame.operation);
      const timeout = new Promise((_, reject) => {
        timer = setTimeout(() => {
          this.fenced.add(frame.device);
          reject(Error('USB deadline expired; completion unknown'));
        }, frame.timeout_ms);
      });
      const reply = Promise.race([running, timeout])
        .catch((error) => {
          this.fenced.add(frame.device);
          return { error: error.message || String(error) };
        })
        .finally(() => clearTimeout(timer));
      // Keep the physical operation in the queue even after reporting timeout.
      const completion = await reply;
      this.onreply?.({
        type: 'reply',
        id: frame.id,
        completion: { devices: [], data: '', transferred: 0, error: null, ...completion },
      });
      await running.catch(() => {});
    });
    this.queues.set(
      frame.device,
      task.catch(() => {}),
    );
    return task;
  }
  async stop() {
    this.stopped = true;
    await Promise.all([...this.queues.values()]);
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
