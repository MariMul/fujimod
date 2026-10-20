import { cp, mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { wasmFixture } from '../tests/wasm-fixture.ts';
import { wasmSessionFixture } from '../tests/wasm-session-fixture.ts';
const source = process.argv[2];
if (!source) throw Error('Usage: node scripts/prepare-wasm-proof.ts <module-directory>');
for (const fault of [false, true]) {
  const output = resolve('public/runtime', fault ? 'wasm-proof-fault' : 'wasm-proof');
  await mkdir(output, { recursive: true });
  const fixture = await wasmFixture(fault);
  if (fault) {
    fixture.manifest.module = '../wasm-proof/camera.mjs';
    fixture.manifest.wasm = '../wasm-proof/camera.wasm';
  } else {
    for (const name of ['camera.mjs', 'camera.wasm'])
      await cp(resolve(source, name), resolve(output, name));
  }
  await writeFile(resolve(output, 'program.dat'), fixture.dat);
  await writeFile(resolve(output, 'wasm.json'), JSON.stringify(fixture.manifest));
  await writeFile(resolve(output, 'inputs.json'), JSON.stringify(fixture.inputs));
}

const output = resolve('public/runtime/wasm-session-proof');
await mkdir(output, { recursive: true });
const fixture = await wasmSessionFixture();
await writeFile(resolve(output, 'program.dat'), fixture.dat);
await writeFile(resolve(output, 'wasm.json'), JSON.stringify(fixture.manifest));
