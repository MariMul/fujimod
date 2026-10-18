import { expect, test } from 'vite-plus/test';
import { retrieveReport, removeReport, mailboxAddress, type ReportMemory } from './transfer';
import { tar, link } from './fixtures.test-helper';
function camera() {
  const address = mailboxAddress(link),
    header = new Uint8Array(64),
    v = new DataView(header.buffer);
  for (const [offset, value] of [
    [0, 0x31425246],
    [4, 1],
    [8, parseInt(link.nonce, 16)],
    [12, tar.length],
    [16, parseInt(link.crc32, 16)],
    [36, 3],
    [40, parseInt(link.commit.slice(0, 8), 16)],
  ])
    v.setUint32(offset, value, true);
  const writes: number[] = [];
  let cursor = 0,
    chunk = new Uint8Array(),
    selection = new Uint8Array();
  const memory: ReportMemory = {
    read: async (at, count) => (at === address ? header.slice() : chunk.slice(0, count)),
    write: async (at, data) => {
      if (at === address + 64) {
        expect(data.length).toBe(32);
        selection = data.slice();
        return;
      }
      expect(at).toBe(address + 20);
      expect(data.length).toBe(4);
      const cmd = new DataView(data.buffer).getUint32(0, true);
      writes.push(cmd);
      if (cmd === 4) {
        const chosen = new DataView(selection.buffer);
        expect(new TextDecoder().decode(selection.slice(0, 12))).toBe('R1234567.TAR');
        v.setUint32(8, chosen.getUint32(24, true), true);
        v.setUint32(12, chosen.getUint32(16, true), true);
        v.setUint32(16, chosen.getUint32(20, true), true);
        cursor = 0;
      } else if (cmd === 5) {
        v.setUint32(8, 0, true);
      } else if (cmd === 2 || cmd === 3) {
        cursor = 0;
        v.setUint32(28, 0, true);
        v.setUint32(32, 0, true);
      } else {
        chunk = tar.slice(cursor, cursor + 16384);
        v.setUint32(28, cursor, true);
        v.setUint32(32, chunk.length, true);
        cursor += chunk.length;
      }
    },
  };
  return { memory, v, writes };
}
test('retrieves exact file bytes through bounded sequential read commands', async () => {
  const { memory, writes } = camera(),
    progress: number[] = [];
  const blob = await retrieveReport(memory, link, (n) => progress.push(n));
  expect(new Uint8Array(await blob.arrayBuffer())).toEqual(tar);
  expect(writes).toEqual([4, 1, 1, 3]);
  expect(progress).toEqual([16384, tar.length]);
});
test('rejects wrong builds, service mode and arbitrary addresses before writing', async () => {
  for (const field of [40, 36]) {
    const { memory, v, writes } = camera();
    v.setUint32(field, field === 36 ? 12 : 0, true);
    await expect(retrieveReport(memory, link, () => {})).rejects.toThrow();
    expect(writes).toEqual([]);
  }
  expect(() => mailboxAddress({ ...link, mailbox: '40040000' })).toThrow();
  expect(() => mailboxAddress({ ...link, mailbox: '405fffe0' })).toThrow();
});

test('selects the saved file even after the export channel has restarted', async () => {
  const { memory, v } = camera();
  v.setUint32(8, 0, true);
  v.setUint32(12, 0, true);
  v.setUint32(16, 0, true);
  const blob = await retrieveReport(memory, link, () => {});
  expect(new Uint8Array(await blob.arrayBuffer())).toEqual(tar);
});

test('delete refuses older builds and wrong captures before writing; checks acknowledgement', async () => {
  const { memory, v, writes } = camera();
  await expect(removeReport(memory, link)).rejects.toThrow('does not support');
  expect(writes).toEqual([]);
  v.setUint32(44, 1, true);
  v.setUint32(8, 0, true);
  await expect(removeReport(memory, link)).rejects.toThrow('requested report');
  expect(writes).toEqual([]);
  v.setUint32(8, parseInt(link.nonce, 16), true);
  await removeReport(memory, link);
  expect(writes).toEqual([5]);
});
