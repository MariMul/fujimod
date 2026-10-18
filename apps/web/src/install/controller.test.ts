import { webcrypto } from 'node:crypto';
import { expect, test, vi } from 'vite-plus/test';
import { Installer, type Card } from './controller';
import { previewAdapter } from './preview';
import { x70Installer } from '../../devices/x70/installer';
vi.stubGlobal('crypto', webcrypto);
function fixture(filesystem: Card['filesystem'] = 'FAT32') {
  const adapter = previewAdapter(filesystem),
    calls: string[] = [];
  for (const name of ['guard', 'card', 'files', 'format', 'write', 'read'] as const) {
    const method = adapter[name].bind(adapter);
    (adapter[name] as Function) = (...args: unknown[]) => {
      calls.push(name);
      return (method as Function)(...args);
    };
  }
  const installer = new Installer(adapter);
  installer.choose(x70Installer);
  return { adapter, calls, installer };
}
test('guard before SD access; both files read back before final guard; menu confirmation remains manual', async () => {
  const { installer, calls } = fixture();
  await installer.connect();
  expect(calls.slice(0, 2)).toEqual(['guard', 'card']);
  await installer.install();
  expect(installer.state.step).toBe('battery');
  expect(installer.state.verified).toEqual(['ffboot.bin', 'ffsys.bin']);
  expect(calls.filter((name) => name === 'write')).toHaveLength(2);
  expect(calls.filter((name) => name === 'read')).toHaveLength(2);
  expect(calls.at(-1)).toBe('guard');
  installer.advance();
  expect(installer.state.step).toBe('boot');
  installer.advance();
  expect(installer.state.step).toBe('confirm');
  installer.advance();
  expect(installer.state.step).toBe('done');
});
test('exFAT refuses install and requires consent before formatting and re-probing FAT32', async () => {
  const { installer, calls } = fixture('exFAT');
  await installer.connect();
  await installer.install();
  await installer.format(false);
  expect(calls).not.toContain('write');
  expect(calls).not.toContain('format');
  await installer.format(true);
  expect(installer.state.card?.filesystem).toBe('FAT32');
  expect(calls.filter((name) => name === 'format')).toHaveLength(1);
  await installer.install();
  expect(installer.state.step).toBe('battery');
});
test('normal USB prompts service reboot without touching settings or card', async () => {
  const { installer, adapter, calls } = fixture();
  adapter.connect = async () => ({
    session: 'normal',
    firmware: x70Installer.id,
    debug: false,
    mode: 'record',
  });
  await installer.connect();
  expect(installer.state.step).toBe('service');
  expect(calls).toEqual([]);
});
test('REC debug sessions never write the JIG-only guard', async () => {
  const { installer, adapter, calls } = fixture();
  adapter.connect = async () => ({
    session: 'rec',
    firmware: x70Installer.id,
    debug: true,
    mode: 'record',
  });
  await installer.connect();
  await installer.install();
  expect(installer.state.step).toBe('battery');
  expect(installer.state.guard).toBe('not-needed');
  expect(calls).not.toContain('guard');
});
test('wrong firmware and failed guard never reach the SD card', async () => {
  const wrong = fixture();
  wrong.adapter.connect = async () => ({
    session: 'wrong',
    firmware: 'xm1-150',
    debug: true,
    mode: 'service',
  });
  await wrong.installer.connect();
  expect(wrong.installer.state.step).toBe('error');
  expect(wrong.calls).toEqual([]);
  const failed = fixture();
  failed.adapter.guard = async () => ({ armed: false, settingsUnchanged: true });
  await failed.installer.connect();
  expect(failed.installer.state.step).toBe('error');
  expect(failed.calls).not.toContain('card');
});
test('source hash failures refuse all writes; readback hash failures withhold battery instructions and retries', async () => {
  const source = fixture(),
    files = source.adapter.files.bind(source.adapter);
  source.adapter.files = async (target) => {
    const result = await files(target);
    result.files[0].sha256 = '0'.repeat(64);
    return result;
  };
  await source.installer.connect();
  await source.installer.install();
  expect(source.installer.state.step).toBe('error');
  expect(source.calls).not.toContain('write');
  const bad = fixture();
  bad.adapter.read = async () => new Uint8Array([1, 2, 3]);
  await bad.installer.connect();
  await bad.installer.install();
  expect(bad.installer.state.step).toBe('error');
  expect(bad.installer.state.message).toContain('Read-back');
  await bad.installer.install();
  expect(bad.calls.filter((name) => name === 'write')).toHaveLength(1);
});
test('card swaps and falsely successful format requests cannot proceed', async () => {
  const swap = fixture();
  await swap.installer.connect();
  const card = swap.adapter.card.bind(swap.adapter);
  swap.adapter.card = async (session) => ({ ...(await card(session)), id: 'replacement' });
  await swap.installer.install();
  expect(swap.installer.state.step).toBe('error');
  expect(swap.calls).not.toContain('write');
  const bad = fixture('exFAT');
  bad.adapter.format = async () => {};
  await bad.installer.connect();
  await bad.installer.format(true);
  expect(bad.installer.state.step).toBe('error');
});
test('disconnect during a write stops before readback; double clicks never repeat the write', async () => {
  const adapter = previewAdapter('FAT32');
  let disconnect!: () => void, finish!: () => void;
  adapter.onDisconnect = (callback) => {
    disconnect = callback;
    return () => {};
  };
  const write = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const read = vi.fn(adapter.read.bind(adapter));
  adapter.write = write;
  adapter.read = read;
  const installer = new Installer(adapter);
  installer.choose(x70Installer);
  await installer.connect();
  const running = installer.install();
  await vi.waitFor(() => expect(write).toHaveBeenCalledTimes(1));
  await installer.install();
  disconnect();
  finish();
  await running;
  expect(installer.state.step).toBe('error');
  expect(read).not.toHaveBeenCalled();
  expect(write).toHaveBeenCalledTimes(1);
});

test('in-camera formatting verifies the exit guard, then requires a fresh connection and card probe', async () => {
  const { installer, adapter, calls } = fixture('exFAT');
  await installer.connect();
  await installer.manualFormat();
  expect(installer.state.step).toBe('manual-format');
  expect(calls.at(-1)).toBe('guard');
  expect(calls).not.toContain('format');
  installer.reconnectFormatted();
  expect(installer.state.step).toBe('connect');
  expect(installer.state.card).toBeUndefined();
  await installer.connect();
  expect(installer.state.card?.filesystem).toBe('exFAT');
  await installer.install();
  expect(calls).not.toContain('write');
  await installer.manualFormat();
  installer.reconnectFormatted();
  await adapter.format('preview', 'preview-card');
  await installer.connect();
  expect(installer.state.card?.filesystem).toBe('FAT32');
});

test('a wrong package refuses writes, and a failed final guard withholds restart instructions', async () => {
  const wrong = fixture(),
    files = wrong.adapter.files.bind(wrong.adapter);
  wrong.adapter.files = async (target) => ({
    ...(await files(target)),
    firmware: 'other-firmware',
  });
  await wrong.installer.connect();
  await wrong.installer.install();
  expect(wrong.installer.state.step).toBe('error');
  expect(wrong.calls).not.toContain('write');
  const final = fixture();
  await final.installer.connect();
  final.adapter.guard = async () => ({ armed: false, settingsUnchanged: true });
  await final.installer.install();
  expect(final.installer.state.verified).toHaveLength(2);
  expect(final.installer.state.step).toBe('error');
  expect(final.installer.state.message).toContain('guard');
});
