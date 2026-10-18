import { expect, test } from 'vite-plus/test';
import { DeviceFiles } from './device-files';
import { fileDevice } from './device-files.test-helper';
const range = (address: number, n: number) => {
  if (address < 0x403a0000 || address + n > 0x403b0000) throw Error('Outside mod memory');
};
test('shared file reader follows the advertised capacity and exposes generic deletion', async () => {
  const data = new Uint8Array(37).fill(42),
    device = fileDevice(data);
  device.word(device.pointers[0], 16);
  const files = new DeviceFiles(device.memory, device.address, 0xaaaaaaaa, range);
  expect(await files.read('C:\\DCIM\\TEST.RAF', 37, () => {})).toEqual(data);
  expect(device.commands).toEqual([1, 2, 2, 2, 3]);
  await files.remove('C:\\DCIM\\TEST.RAF');
  expect(device.commands.at(-1)).toBe(7);
});
test('rejects open handles and invalid pointers before writing', async () => {
  const device = fileDevice(new Uint8Array(8));
  device.word(device.pointers[9], 7);
  await expect(
    new DeviceFiles(device.memory, device.address, 0xaaaaaaaa, range).read(
      'C:\\TEST.RAF',
      8,
      () => {},
    ),
  ).rejects.toThrow('open');
  expect(device.writes).toEqual([]);
  device.word(device.pointers[9], 0xffffffff);
  device.word(device.address + 12 + 6 * 4, 0x40000000);
  await expect(
    new DeviceFiles(device.memory, device.address, 0xaaaaaaaa, range).read(
      'C:\\TEST.RAF',
      8,
      () => {},
    ),
  ).rejects.toThrow('Outside');
  expect(device.writes).toEqual([]);
});
test('detects a competing operation before sending another command', async () => {
  const device = fileDevice(new Uint8Array(8));
  const files = new DeviceFiles(device.memory, device.address, 0xaaaaaaaa, range);
  await files.read('C:\\TEST.RAF', 8, () => {});
  device.word(device.pointers[7], 99);
  await expect(files.remove('C:\\TEST.RAF')).rejects.toThrow('intervened');
  expect(device.commands).toEqual([1, 2, 3]);
});

test('a short read closes the completed operation rather than retaining an open handle', async () => {
  const device = fileDevice(new Uint8Array(3));
  await expect(
    new DeviceFiles(device.memory, device.address, 0xaaaaaaaa, range).read(
      'C:\\TEST.RAF',
      8,
      () => {},
    ),
  ).rejects.toThrow('ended');
  expect(device.commands).toEqual([1, 2, 4]);
});

test('a pending operation prevents even the path from being replaced', async () => {
  const device = fileDevice(new Uint8Array(8));
  device.word(device.pointers[2], 1);
  await expect(
    new DeviceFiles(device.memory, device.address, 0xaaaaaaaa, range).read(
      'C:\\TEST.RAF',
      8,
      () => {},
    ),
  ).rejects.toThrow('busy');
  expect(device.writes).toEqual([]);
});
