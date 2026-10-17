import { type ReportLink, validateReport } from './report-link';
export interface ReportMemory {
  read(address: number, count: number): Promise<Uint8Array>;
  write(address: number, bytes: Uint8Array<ArrayBuffer>): Promise<void>;
}
const HEADER = 64,
  CHUNK = 16384,
  MAGIC = 0x31425246;
export function mailboxAddress(link: ReportLink): number {
  const address = parseInt(link.mailbox, 16),
    physical = address - 0x40000000;
  // Only the X70 1.10 checked mod region, including the entire fixed channel.
  if (physical < 0x390000 || physical + HEADER + CHUNK > 0x600000 || address % 32)
    throw Error('Invalid report export channel.');
  return address;
}
export async function retrieveReport(
  memory: ReportMemory,
  link: ReportLink,
  progress: (bytes: number) => void,
): Promise<Blob> {
  const address = mailboxAddress(link),
    bytes = new Uint8Array(link.bytes);
  async function header(selected = true) {
    const data = await memory.read(address, HEADER);
    if (data.length !== HEADER) throw Error('Incomplete export reply.');
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
    if (
      view.getUint32(0, true) !== MAGIC ||
      view.getUint32(4, true) !== 1 ||
      view.getUint32(40, true) !== parseInt(link.commit.slice(0, 8), 16) ||
      (selected &&
        (view.getUint32(8, true) !== parseInt(link.nonce, 16) ||
          view.getUint32(12, true) !== link.bytes ||
          view.getUint32(16, true) !== parseInt(link.crc32, 16)))
    )
      throw Error('This camera no longer has the requested report.');
    if (view.getUint32(36, true) === 12)
      throw Error('Report retrieval is unavailable in service mode.');
    return view;
  }
  async function command(command: number) {
    await header(command !== 4); // Validate identity before every write to the mod-owned command word.
    const data = new Uint8Array(4);
    new DataView(data.buffer).setUint32(0, command, true);
    await memory.write(address + 20, data);
    const deadline = performance.now() + 15000;
    for (;;) {
      const view = await header(command !== 4),
        status = view.getInt32(24, true);
      if (!view.getUint32(20, true) && status !== 1) {
        if (status) throw Error(`Camera could not read the report (${status}).`);
        if (command === 4) await header();
        return view;
      }
      if (performance.now() >= deadline) throw Error('Camera report retrieval timed out.');
      await new Promise((resolve) => setTimeout(resolve, 40));
    }
  }
  await header(false);
  const selection = new Uint8Array(32),
    fields = new DataView(selection.buffer);
  selection.set(new TextEncoder().encode(link.path.split('\\').at(-1)!));
  fields.setUint32(16, link.bytes, true);
  fields.setUint32(20, parseInt(link.crc32, 16), true);
  fields.setUint32(24, parseInt(link.nonce, 16), true);
  await memory.write(address + HEADER, selection);
  await command(4);
  let offset = 0;
  while (offset < bytes.length) {
    const view = await command(1),
      count = view.getUint32(32, true);
    if (
      view.getUint32(28, true) !== offset ||
      !count ||
      count > CHUNK ||
      offset + count > bytes.length
    )
      throw Error('Unexpected report chunk.');
    const chunk = await memory.read(address + HEADER, count);
    if (chunk.length !== count) throw Error('Incomplete report chunk.');
    bytes.set(chunk, offset);
    offset += count;
    progress(offset);
  }
  await command(3);
  return validateReport(bytes, link);
}
