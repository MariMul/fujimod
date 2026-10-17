import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { boundedMemoryModule } from './wasm-growth.ts';

test('growth uses the runtime view refresh with bounded slack and an exact retry', async () => {
  const sizes=[];
  const settings={growMemory:bytes=>{sizes.push(bytes);return sizes.length>1;},
    instantiateWasm:imports=>imports.env.emscripten_resize_heap(536870913)};
  const load=boundedMemoryModule(async config=>config.instantiateWasm({env:{emscripten_resize_heap:()=>{throw Error('Unbounded growth');}}}));
  assert.equal(await load(settings),true);
  assert.deepEqual(sizes,[541130752,536936448]);
});

test('a changed runtime integration fails before allocating guest RAM', async () => {
  const load=boundedMemoryModule(async settings=>settings.instantiateWasm({env:{}}));
  await assert.rejects(load({instantiateWasm:()=>{throw Error('Must validate imports first');}}),/heap-growth ABI/);
});
