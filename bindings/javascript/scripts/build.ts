import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join, resolve } from 'node:path';
const root = fileURLToPath(new URL('../../..', import.meta.url));
const pkg = fileURLToPath(new URL('../pkg', import.meta.url));
const target = process.env.CARGO_TARGET_DIR
  ? resolve(root, process.env.CARGO_TARGET_DIR)
  : join(root, 'target');
function run(command: string, args: string[]) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
const testing = process.argv.includes('--testing');
run('cargo', [
  'build',
  '-p',
  'fujimod-javascript',
  '--profile',
  'dist',
  '--target',
  'wasm32-unknown-unknown',
  ...(testing ? ['--features', 'testing'] : []),
]);
run(process.env.WASM_BINDGEN ?? join(root, 'target/tooling/bin/wasm-bindgen'), [
  join(target, 'wasm32-unknown-unknown/dist/fujimod_javascript.wasm'),
  '--target',
  'web',
  '--out-dir',
  pkg,
  '--out-name',
  'fujimod',
]);

run('cargo', ['run', '-p', 'fujimod-bindgen', '--', '--out', pkg]);
