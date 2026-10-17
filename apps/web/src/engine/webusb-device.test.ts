import { expect, test } from 'vite-plus/test';
import { EmulatedUsbDevice, emulatedUsb, type UsbBus } from './webusb-device';
const descriptor = Uint8Array.from([
  18, 1, 0, 2, 0, 0, 0, 64, 0xcb, 4, 0x80, 0xff, 0, 1, 0, 0, 0, 1,
]);
const configuration = Uint8Array.from([
  9, 2, 32, 0, 1, 1, 0, 0x80, 50, 9, 4, 2, 3, 2, 0xff, 0, 0, 0, 7, 5, 0x82, 2, 64, 0, 0, 7, 5, 1, 2,
  64, 0, 0,
]);
function fixture() {
  const calls: unknown[][] = [];
  const bus: UsbBus = {
    async controlIn(setup, length) {
      calls.push(['control-in', setup, length]);
      return Uint8Array.from([5, 6]);
    },
    async controlOut(setup, data) {
      calls.push(['control-out', setup, data]);
      return data.length;
    },
    async bulkIn(endpoint, length) {
      calls.push(['bulk-in', endpoint, length]);
      return Uint8Array.from([7, 8]);
    },
    async bulkOut(endpoint, data) {
      calls.push(['bulk-out', endpoint, data]);
      return data.length;
    },
  };
  return { device: new EmulatedUsbDevice(bus, descriptor, [configuration]), calls };
}
test('WebUSB descriptors, interface ownership, and endpoint addresses come from firmware', async () => {
  const { device, calls } = fixture();
  expect(device.vendorId).toBe(0x04cb);
  expect(device.productId).toBe(0xff80);
  expect(device.configurations[0].interfaces[0].alternates[0].alternateSetting).toBe(3);
  await expect(device.transferIn(2, 64)).rejects.toThrow('closed');
  await device.open();
  await device.selectConfiguration(1);
  await expect(device.transferIn(2, 64)).rejects.toThrow('not claimed');
  await device.claimInterface(2);
  await device.selectAlternateInterface(2, 3);
  await expect(device.transferIn(1, 64)).rejects.toThrow('not claimed');
  const data = await device.transferIn(2, 64);
  expect([...new Uint8Array(data.data.buffer)]).toEqual([7, 8]);
  const source = Uint8Array.from([99, 10, 11, 99]);
  expect(await device.transferOut(1, source.subarray(1, 3))).toEqual({
    status: 'ok',
    bytesWritten: 2,
  });
  source.fill(0);
  expect(calls.at(-2)).toEqual(['bulk-in', 0x82, 64]);
  expect(calls.at(-1)).toEqual(['bulk-out', 1, Uint8Array.from([10, 11])]);
  await device.releaseInterface(2);
  await expect(device.transferOut(1, new Uint8Array())).rejects.toThrow('not claimed');
  const count = calls.length;
  await device.close();
  expect(calls).toHaveLength(count);
});
test('control transfers preserve their setup and opaque payload; virtual picker filters devices', async () => {
  const { device, calls } = fixture();
  await device.open();
  const setup = {
    requestType: 'vendor',
    recipient: 'other',
    request: 0,
    value: 0,
    index: 0,
  } as const;
  await device.controlTransferOut(setup, Uint8Array.from([0xc0, 0, 0, 0]));
  expect(calls.at(-1)).toEqual(['control-out', setup, Uint8Array.from([0xc0, 0, 0, 0])]);
  const incoming = await device.controlTransferIn(setup, 2);
  expect(incoming.status).toBe('ok');
  expect([...new Uint8Array(incoming.data.buffer)]).toEqual([5, 6]);
  const usb = emulatedUsb(device);
  expect(await usb.getDevices()).toEqual([device]);
  expect(await usb.requestDevice({ filters: [{ vendorId: 0x04cb, productId: 0xff80 }] })).toBe(
    device,
  );
  await expect(usb.requestDevice({ filters: [{ vendorId: 1 }] })).rejects.toThrow('does not match');
});
test('malformed firmware descriptors fail instead of inventing endpoint capabilities', () => {
  const { device } = fixture();
  const bad = configuration.slice();
  bad[9] = 0;
  expect(() => new EmulatedUsbDevice({} as UsbBus, descriptor, [bad])).toThrow('extent');
  expect(device.configurations[0].interfaces[0].alternates[0].endpoints).toHaveLength(2);
});
