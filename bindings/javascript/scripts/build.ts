import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
const root = fileURLToPath(new URL('../../..', import.meta.url));
const pkg = fileURLToPath(new URL('../pkg', import.meta.url));
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
  '--release',
  '--target',
  'wasm32-unknown-unknown',
  ...(testing ? ['--features', 'testing'] : []),
]);
run(process.env.WASM_BINDGEN ?? join(root, 'target/tooling/bin/wasm-bindgen'), [
  join(root, 'target/wasm32-unknown-unknown/release/fujimod_javascript.wasm'),
  '--target',
  'web',
  '--out-dir',
  pkg,
  '--out-name',
  'fujimod',
]);

run('cargo', ['run', '-p', 'fujimod-bindgen', '--', '--out', pkg]);
