import { DeviceFiles, type FileMemory } from '../device-files';
import { type ReportLink, validateReport } from './report-link';
/** X70 1.10 policy belongs here, outside the shared file protocol. */
function files(memory: FileMemory, link: ReportLink) {
  function range(address: number, count: number) {
    if (
      !Number.isSafeInteger(address) ||
      address < 0 ||
      address >= 0x80000000 ||
      (address & ~0x40000000) < 0x390000 ||
      (address & ~0x40000000) + count > 0x600000 ||
      count < 0
    )
      throw Error('Invalid file-service memory range.');
  }
  const address = parseInt(link.files, 16);
  if (address % 32) throw Error('Unaligned file-service descriptor.');
  const uncached: FileMemory = {
    read: (at, count) => {
      range(at, count);
      return memory.read(at | 0x40000000, count);
    },
    write: (at, bytes) => {
      range(at, bytes.length);
      return memory.write(at | 0x40000000, bytes);
    },
  };
  return new DeviceFiles(uncached, address, parseInt(link.commit.slice(0, 8), 16), range);
}
export async function retrieveReport(
  memory: FileMemory,
  link: ReportLink,
  progress: (bytes: number) => void,
): Promise<Blob> {
  return validateReport(await files(memory, link).read(link.path, link.bytes, progress), link);
}
export async function removeReport(memory: FileMemory, link: ReportLink): Promise<void> {
  await files(memory, link).remove(link.path);
}
