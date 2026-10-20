import assert from 'node:assert/strict';
import test from 'node:test';
import { ShareHost, type PeerChannel } from './peer.ts';
import type { SharedDevice, ExecuteMessage } from './types.ts';

function fixture() {
  const operations: string[] = [];
  const device: SharedDevice = {
    descriptor: {
      id: 'camera',
      vendor_id: 0x4cb,
      product_id: 0xff80,
      manufacturer: null,
      product: null,
      serial: null,
      interfaces: [],
    },
    async execute(op) {
      operations.push(op.op);
      return {};
    },
    async close() {
      operations.push('local close');
    },
  };
  const host = Object.create(ShareHost.prototype) as ShareHost;
  Object.assign(host, {
    device,
    descriptor: device.descriptor,
    peers: new Map(),
    callbacks: {},
    closed: false,
    fenced: false,
  });
  const request = (op: string): ExecuteMessage => ({
    type: 'execute',
    seq: 1,
    device: 'camera',
    timeout_ms: 5000,
    op: { op },
  });
  const run = (op: string) => host.run('remote', { ended: false } as PeerChannel, request(op));
  return { host, device, operations, run };
}

await test('local sessions and remote opens share one ownership slot, including asynchronous release', async () => {
  const f = fixture();
  await f.run('open');
  assert.throws(() => f.host.claimLocal(), /in use by a remote controller/);
  assert.deepEqual(f.operations, ['open']);
  await f.run('close');
  const local = f.host.claimLocal();
  await assert.rejects(f.run('open'), /in use/);
  let finish!: () => void;
  f.device.close = () =>
    new Promise<void>((resolve) => {
      finish = resolve;
    });
  const releasing = local.release();
  await assert.rejects(f.run('open'), /in use/);
  finish();
  await releasing;
  await f.run('open');
  await local.release(); // A stale release cannot drop the next owner's claim.
  assert.equal(f.host.owner, 'remote');
});

await test('abandoned, failed-close, offline and stopped shares refuse local sessions', async () => {
  for (const mode of ['abandon', 'failed-close', 'offline', 'stopped']) {
    const f = fixture();
    if (mode === 'offline') f.host.device = undefined;
    else if (mode === 'stopped') f.host.closed = true;
    else {
      const local = f.host.claimLocal();
      if (mode === 'abandon') local.abandon();
      else {
        f.device.close = async () => {
          throw Error('close failed');
        };
        await assert.rejects(local.release(), /close failed/);
      }
      await assert.rejects(f.run('open'), /completion unknown/);
    }
    assert.throws(() => f.host.claimLocal(), /offline|completion unknown/);
  }
});
