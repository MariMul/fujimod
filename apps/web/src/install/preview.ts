import { sha256, type BootFile, type Card, type InstallerAdapter } from './controller';
/** UI test double only. It never selects USB or executes firmware. */
export function previewAdapter(filesystem: Card['filesystem'] = 'exFAT'): InstallerAdapter {
  let format = filesystem;
  const disk = new Map<string, Uint8Array<ArrayBuffer>>();
  return {
    available: true,
    canFormat: true,
    async connect(target) {
      return { session: 'preview', firmware: target.id, debug: true, mode: 'service' };
    },
    async guard() {
      return { armed: true, settingsUnchanged: true };
    },
    async card() {
      return {
        id: 'preview-card',
        filesystem: format,
        capacity: 32 * 1024 ** 3,
        free: 24 * 1024 ** 3,
      };
    },
    async files(target) {
      const files: BootFile[] = [];
      for (const name of ['ffboot.bin', 'ffsys.bin'] as const) {
        const data = new TextEncoder().encode(`Preview only: ${name}`);
        files.push({ name, data, sha256: await sha256(data) });
      }
      return { firmware: target.id, files };
    },
    async format() {
      disk.clear();
      format = 'FAT32';
    },
    async write(_session, _card, file) {
      disk.set(file.name, file.data.slice());
    },
    async read(_session, _card, name) {
      const bytes = disk.get(name);
      if (!bytes) throw Error('Preview file not found');
      return bytes.slice();
    },
    abandon() {},
  };
}
