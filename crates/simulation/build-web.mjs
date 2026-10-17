import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
const root = fileURLToPath(new URL('../../', import.meta.url));
const out = join(root, 'bindings/javascript/pkg/simulation');
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run('cargo', ['build', '--locked', '-p', 'fujimod-simulation', '--profile', 'simulation-web', '--target', 'wasm32-unknown-unknown']);
run(process.env.WASM_BINDGEN ?? join(root, 'target/tooling/bin/wasm-bindgen'), [join(root, 'target/wasm32-unknown-unknown/simulation-web/fujimod_simulation.wasm'), '--target', 'web', '--out-dir', out, '--out-name', 'fujimod_simulation']);

if (process.argv.includes('--optimize')) run(join(root, 'apps/web/node_modules/.bin/wasm-opt'), [join(out, 'fujimod_simulation_bg.wasm'), '-Oz', '--all-features', '-o', join(out, 'fujimod_simulation_bg.wasm')]);
