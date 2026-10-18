import { assert, test } from 'vite-plus/test';
import { faultValue } from './fault';

test('fault details retain scalar values and structured diagnostics', () => {
  assert.equal(faultValue(0), '0');
  assert.equal(faultValue(false), 'false');
  assert.equal(faultValue(null), '');
  assert.equal(faultValue('0xff00'), '0xff00');
  assert.equal(
    faultValue({ operation: 'write', address: 32 }),
    '{"operation":"write","address":32}',
  );
  const cyclic: { self?: object } = {};
  cyclic.self = cyclic;
  assert.equal(faultValue(cyclic), 'Unserializable fault detail');
});
