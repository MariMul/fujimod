import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { readFileSync, statSync, writeFileSync } from 'node:fs';
const root = fileURLToPath(new URL('../../', import.meta.url));
const out = join(root, 'bindings/javascript/pkg/simulation');
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run('cargo', ['build', '--locked', '-p', 'fujimod-simulation', '--profile', 'simulation-web', '--target', 'wasm32-unknown-unknown']);
const source = join(root, 'target/wasm32-unknown-unknown/simulation-web/fujimod_simulation.wasm');
const info = statSync(source);
const stamp = JSON.stringify({ size: info.size, modified: info.mtimeMs,
  script: statSync(fileURLToPath(import.meta.url)).mtimeMs });
const stampFile = join(out, 'build.json');
try {
  if (readFileSync(stampFile, 'utf8') === stamp &&
      statSync(join(out, 'fujimod_simulation_bg.wasm')).size > 0 &&
      statSync(join(out, 'fujimod_simulation.js')).size > 0) process.exit(0);
} catch { /* First build, or generated output was removed. */ }
run(process.env.WASM_BINDGEN ?? join(root, 'target/tooling/bin/wasm-bindgen'), [source, '--target', 'web', '--out-dir', out, '--out-name', 'fujimod_simulation']);

run(process.env.WASM_OPT ?? join(root, 'apps/web/node_modules/.bin/wasm-opt'), [join(out, 'fujimod_simulation_bg.wasm'), '-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-reference-types', '--enable-multivalue', '--enable-mutable-globals', '-o', join(out, 'fujimod_simulation_bg.wasm')]);
const bytes = statSync(join(out, 'fujimod_simulation_bg.wasm')).size;
if (bytes > 64 * 1024 * 1024) throw Error(`Simulation Wasm exceeds its 64 MiB budget: ${bytes} bytes`);
console.log(`Optimized simulation Wasm: ${bytes} bytes`);
writeFileSync(stampFile, stamp);
