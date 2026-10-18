import assert from 'node:assert/strict';
import test from 'node:test';
import { ShareHost } from './peer.js';

// The signaling socket is not exercised: these check the offline state machine only.
function host() {
  const events = [];
  const device = { descriptor: { id: 'cam' }, close: async () => events.push('closed') };
  const share = Object.create(ShareHost.prototype);
  Object.assign(share, {
    ticket: {},
    device,
    descriptor: device.descriptor,
    offline: undefined,
    battery: undefined,
    callbacks: { state: (status) => events.push(status) },
    peers: new Map(),
    owner: undefined,
    closed: false,
    fenced: true,
  });
  return { share, events };
}

await test('a lost camera keeps the share, reports why, and comes back on the same share', () => {
  const { share, events } = host();
  share.battery = { level: 'LOW', at: Date.now() };
  share.cameraLost('battery');
  assert.equal(share.device, undefined);
  assert.deepEqual(
    { ...share.status(), battery_age_ms: 0 },
    { online: false, reason: 'battery', battery: 'LOW', battery_age_ms: 0 },
  );
  assert.deepEqual(
    events.slice(0, 2).map((e) => (typeof e === 'string' ? e : e.online)),
    ['closed', false],
  );
  const returned = { descriptor: { id: 'cam2' }, close: async () => {} };
  share.cameraReturned(returned);
  assert.equal(share.device, returned);
  assert.equal(share.descriptor.id, 'cam2');
  assert.equal(share.status().online, true);
  assert.equal(share.fenced, false);
});
