import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { boundedMemoryModule } from './wasm-growth.ts';

test('growth uses the runtime view refresh with bounded slack and an exact retry', async () => {
  const sizes: number[] = [];
  const settings = {
    growMemory: (bytes: number) => {
      sizes.push(bytes);
      return Number(sizes.length > 1);
    },
    instantiateWasm: (imports: { env: Record<string, unknown> }) =>
      (imports.env.emscripten_resize_heap as (bytes: number) => number)(536870913),
  };
  const load = boundedMemoryModule(async (config) =>
    config.instantiateWasm(
      {
        env: {
          emscripten_resize_heap: () => {
            throw Error('Unbounded growth');
          },
        },
      },
      undefined,
    ),
  );
  assert.equal(await load(settings), 1);
  assert.deepEqual(sizes, [541130752, 536936448]);
});

test('a changed runtime integration fails before allocating guest RAM', async () => {
  const load = boundedMemoryModule(async (settings) =>
    settings.instantiateWasm({ env: {} }, undefined),
  );
  await assert.rejects(
    load({
      instantiateWasm: () => {
        throw Error('Must validate imports first');
      },
    }),
    /heap-growth ABI/,
  );
});
