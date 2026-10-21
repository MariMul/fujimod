import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  accessSync,
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { devNull } from 'node:os';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { gzipSync } from 'node:zlib';
import { parseArgs } from 'node:util';

export const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export const defaultOutput = resolve(root, '../../target/emulator');
const unicornLicenses = [
  'COPYING',
  'COPYING.LGPL2',
  'COPYING_GLIB',
  'qemu/COPYING',
  'qemu/COPYING.LIB',
  'qemu/LICENSE',
];

export interface BuildOptions {
  out: string;
  stagePackage: boolean;
  profiling: boolean;
  rustSimd128: boolean;
  cSimd128: boolean;
  lto?: string;
  optLevel?: string;
  codegenUnits?: number;
  allocator: 'dlmalloc' | 'emmalloc';
}

export interface BuildPlan {
  env: NodeJS.ProcessEnv;
  rustFlags: string[];
  cFlags: string[];
  rust: string[];
  cmakeConfigure: string[];
  cmakeBuild: string[];
  link: string[];
}

export function shellWords(value: string): string[] {
  const words: string[] = [];
  let word = '';
  let started = false;
  let quote: string | undefined;
  for (let index = 0; index < value.length; index++) {
    const character = value[index]!;
    if (quote) {
      if (character === quote) {
        quote = undefined;
      } else if (character === '\\' && quote === '"') {
        const next = value[++index];
        if (next === undefined) throw new Error('Trailing flag escape.');
        word += next === '"' || next === '\\' ? next : `\\${next}`;
      } else {
        word += character;
      }
    } else if (character === '"' || character === "'") {
      quote = character;
      started = true;
    } else if (character === '\\') {
      const next = value[++index];
      if (next === undefined) throw new Error('Trailing flag escape.');
      word += next;
      started = true;
    } else if (' \t\r\n'.includes(character)) {
      if (started) {
        words.push(word);
        word = '';
        started = false;
      }
    } else {
      word += character;
      started = true;
    }
  }
  if (quote) throw new Error('Unclosed flag quote.');
  if (started) words.push(word);
  return words;
}

export function options(args: string[], base = root): BuildOptions {
  const parsed = parseArgs({
    args,
    options: {
      out: { type: 'string', default: defaultOutput },
      'stage-package': { type: 'boolean', default: false },
      profiling: { type: 'boolean', default: false },
      'rust-simd128': { type: 'boolean', default: false },
      'c-simd128': { type: 'boolean', default: false },
      lto: { type: 'string' },
      'opt-level': { type: 'string' },
      'codegen-units': { type: 'string' },
      allocator: { type: 'string', default: 'dlmalloc' },
    },
  });
  const values = parsed.values;
  if (values.lto !== undefined && !['thin', 'fat', 'off'].includes(values.lto))
    throw new Error('Invalid --lto value.');
  if (
    values['opt-level'] !== undefined &&
    !['0', '1', '2', '3', 's', 'z'].includes(values['opt-level'])
  )
    throw new Error('Invalid --opt-level value.');
  const codegenUnits =
    values['codegen-units'] === undefined ? undefined : Number(values['codegen-units']);
  if (codegenUnits !== undefined && (!Number.isSafeInteger(codegenUnits) || codegenUnits < 1))
    throw new Error('--codegen-units must be a positive integer.');
  if (values.allocator !== 'dlmalloc' && values.allocator !== 'emmalloc')
    throw new Error('Invalid --allocator value.');
  return {
    out: resolve(base, values.out),
    stagePackage: values['stage-package'],
    profiling: values.profiling,
    rustSimd128: values['rust-simd128'],
    cSimd128: values['c-simd128'],
    lto: values.lto,
    optLevel: values['opt-level'],
    codegenUnits,
    allocator: values.allocator,
  };
}

export function buildPlan(selected: BuildOptions, source: NodeJS.ProcessEnv): BuildPlan {
  const out = resolve(selected.out);
  const target = join(out, 'rust');
  const unicorn = join(out, 'unicorn');
  const env: NodeJS.ProcessEnv = {
    ...source,
    CARGO_TARGET_DIR: target,
    CAMERA_UNICORN_STATIC_DIR: unicorn,
  };
  if (selected.lto !== undefined) env.CARGO_PROFILE_RELEASE_LTO = selected.lto;
  if (selected.optLevel !== undefined) env.CARGO_PROFILE_RELEASE_OPT_LEVEL = selected.optLevel;
  if (selected.codegenUnits !== undefined)
    env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS = String(selected.codegenUnits);

  const rustFlags =
    env.CARGO_ENCODED_RUSTFLAGS !== undefined
      ? env.CARGO_ENCODED_RUSTFLAGS.split('\x1f')
      : shellWords(env.RUSTFLAGS ?? '');
  if (selected.rustSimd128) rustFlags.push('-C', 'target-feature=+simd128');
  env.CARGO_ENCODED_RUSTFLAGS = rustFlags.join('\x1f');
  delete env.RUSTFLAGS;

  const simdFlags = selected.rustSimd128 || selected.cSimd128 ? ['-msimd128'] : [];
  const cFlags = [
    '-fwasm-exceptions',
    '-sSUPPORT_LONGJMP=wasm',
    '-DCONFIG_TCI_WASM_TIER',
    ...(selected.cSimd128 ? ['-msimd128'] : []),
  ];
  const rust = [
    'cargo',
    '-Zbuild-std',
    'build',
    '--locked',
    '-j2',
    '--release',
    '--lib',
    '--target',
    'wasm32-unknown-emscripten',
    '--manifest-path',
    join(root, 'emulator/Cargo.toml'),
  ];
  const cmakeConfigure = [
    'emcmake',
    'cmake',
    '-S',
    join(root, '../../crates/emulator-cpu-unicorn/vendor/unicorn'),
    '-B',
    unicorn,
    '-G',
    'Ninja',
    '-DCMAKE_BUILD_TYPE=Release',
    '-DUNICORN_ARCH=arm',
    '-DUNICORN_TCG_INTERPRETER=ON',
    '-DUNICORN_BUILD_TESTS=OFF',
    '-DUNICORN_INSTALL=OFF',
    '-DUNICORN_LEGACY_STATIC_ARCHIVE=OFF',
    '-DBUILD_SHARED_LIBS=OFF',
    `-DCMAKE_C_FLAGS=${cFlags.join(' ')}`,
  ];
  const cmakeBuild = ['ninja', '-j2', '-C', unicorn, 'unicorn'];
  const link = [
    'emcc',
    join(target, 'wasm32-unknown-emscripten/release/libfujimod_emulator_javascript.a'),
    '-o',
    join(out, 'camera.mjs'),
    '-O3',
    '--no-entry',
    ...simdFlags,
    '-fwasm-exceptions',
    '-sSUPPORT_LONGJMP=wasm',
    '-sMODULARIZE=1',
    '-sEXPORT_NAME=createEmulator',
    '-sENVIRONMENT=web,worker,node',
    '-sWASM_BIGINT=1',
    '-sALLOW_MEMORY_GROWTH=1',
    '-sINITIAL_MEMORY=33554432',
    '-sMAXIMUM_MEMORY=2147483648',
    '-sSTACK_SIZE=2097152',
    '-sFILESYSTEM=0',
    '-sNO_EXIT_RUNTIME=1',
    '-sALLOW_TABLE_GROWTH=1',
    `-sMALLOC=${selected.allocator}`,
    '--js-library',
    join(out, 'jit/host.js'),
    '--pre-js',
    join(out, 'jit/loader.js'),
    `-sEXPORTED_FUNCTIONS=@${join(root, 'emulator/exports.json')}`,
    '-sEXPORTED_RUNTIME_METHODS=HEAPU8',
    ...(selected.profiling ? ['--profiling-funcs'] : []),
  ];
  return { env, rustFlags, cFlags, rust, cmakeConfigure, cmakeBuild, link };
}

export function pinnedToolchain(): { nightly: string; emsdk: string } {
  const config = readFileSync(resolve(root, '../../mise.toml'), 'utf8');
  const nightly = config.match(/version\s*=\s*"(nightly-[0-9-]+)"/)?.[1];
  const emsdk = config.match(/^emsdk\s*=\s*"([^"]+)"/m)?.[1];
  if (!nightly || !emsdk)
    throw new Error('Public Mise config must pin nightly Rust and Emscripten.');
  return { nightly, emsdk };
}

function requireTool(tool: string): void {
  const pathSeparator = sep === '\\' ? ';' : ':';
  const available = (process.env.PATH ?? '').split(pathSeparator).some((directory) => {
    try {
      accessSync(join(directory, tool), constants.X_OK);
      return true;
    } catch {
      return false;
    }
  });
  if (!available) throw new Error(`${tool} is required; install the pinned tools in mise.toml.`);
}

function capture(args: string[], cwd = root): string {
  const result = spawnSync(args[0]!, args.slice(1), { cwd, encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || `${args[0]} failed.`);
  return result.stdout;
}

function run(args: string[], env: NodeJS.ProcessEnv): void {
  console.log('+', args.join(' '));
  const result = spawnSync(args[0]!, args.slice(1), { cwd: root, env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${args[0]} exited ${result.status ?? result.signal}.`);
}

export function validateToolchain(pinned = pinnedToolchain()): void {
  const emccVersion = capture(['emcc', '--version']).split('\n')[0] ?? '';
  if (!emccVersion.includes(` ${pinned.emsdk}`))
    throw new Error(`Expected Emscripten ${pinned.emsdk}; got ${emccVersion}.`);
  const installedToolchains = capture(['rustup', 'toolchain', 'list']);
  if (!installedToolchains.split('\n').some((line) => line.startsWith(`${pinned.nightly}-`)))
    throw new Error(`Expected installed Rust toolchain ${pinned.nightly}.`);
  const rustcVersion = capture(['rustup', 'run', pinned.nightly, 'rustc', '--version']);
  if (!rustcVersion.startsWith('rustc '))
    throw new Error(`Could not run Rust toolchain ${pinned.nightly}: ${rustcVersion.trim()}.`);
}

function validateSources(): void {
  const vendor = join(root, '../../crates/emulator-cpu-unicorn/vendor/unicorn');
  for (const path of [
    join(root, 'emulator/Cargo.toml'),
    join(root, 'emulator/exports.json'),
    join(root, 'emulator/camera_wasm.h'),
    join(root, 'jit/host.ts'),
    join(root, 'jit/loader.ts'),
    ...unicornLicenses.map((name) => join(vendor, name)),
  ]) {
    if (!existsSync(path)) throw new Error(`Required emulator build input is missing: ${path}`);
  }
}

function stageRuntimeNotices(out: string): void {
  copyFileSync(join(root, 'NOTICE'), join(out, 'NOTICE'));
  const licenseDirectory = join(out, 'licenses');
  const vendor = join(root, '../../crates/emulator-cpu-unicorn/vendor/unicorn');
  mkdirSync(licenseDirectory, { recursive: true });
  for (const name of unicornLicenses) {
    const fileName = name.replaceAll('/', '-');
    copyFileSync(join(vendor, name), join(licenseDirectory, fileName));
  }
}

function stagePackage(out: string): void {
  const destination = join(root, 'dist/emulator');
  mkdirSync(destination, { recursive: true });
  for (const name of ['camera.mjs', 'camera.wasm', 'build.json', 'NOTICE']) {
    copyFileSync(join(out, name), join(destination, name));
  }
  const licenseDirectory = join(destination, 'licenses');
  mkdirSync(licenseDirectory, { recursive: true });
  for (const name of unicornLicenses) {
    const fileName = name.replaceAll('/', '-');
    copyFileSync(join(out, 'licenses', fileName), join(licenseDirectory, fileName));
  }
}

export function build(selected: BuildOptions) {
  validateSources();
  const pinned = pinnedToolchain();
  for (const tool of [
    'emcc',
    'emcmake',
    'cmake',
    'ninja',
    'cargo',
    'rustc',
    'rustup',
    'node',
    'git',
  ])
    requireTool(tool);
  validateToolchain(pinned);

  const plan = buildPlan(selected, process.env);
  const out = resolve(selected.out);
  mkdirSync(out, { recursive: true });
  const unicornSource = join(root, '../../crates/emulator-cpu-unicorn/vendor/unicorn');
  const cache = join(out, 'unicorn/CMakeCache.txt');
  if (existsSync(cache)) {
    const configured = readFileSync(cache, 'utf8').match(
      /^CMAKE_HOME_DIRECTORY:INTERNAL=(.*)$/m,
    )?.[1];
    if (configured && resolve(configured) !== resolve(unicornSource))
      rmSync(join(out, 'unicorn'), { recursive: true, force: true });
  }

  run(
    [process.execPath, join(root, 'jit/build.mts'), join(root, 'jit'), join(out, 'jit')],
    process.env,
  );
  const start = performance.now();
  run(plan.cmakeConfigure, plan.env);
  run(plan.cmakeBuild, plan.env);
  const unicornSeconds = (performance.now() - start) / 1000;
  run(['cargo', `+${pinned.nightly}`, ...plan.rust.slice(1)], plan.env);
  const rustSeconds = (performance.now() - start) / 1000 - unicornSeconds;
  run(
    ['emcc', '-x', 'c', '-fsyntax-only', '-include', join(root, 'emulator/camera_wasm.h'), devNull],
    plan.env,
  );
  run(plan.link, plan.env);
  const totalSeconds = (performance.now() - start) / 1000;
  const wasm = readFileSync(join(out, 'camera.wasm'));
  const loader = readFileSync(join(out, 'camera.mjs'));
  const hash = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex');
  const vendor = join(root, '../../crates/emulator-cpu-unicorn/vendor/unicorn');
  const dirty = capture(['git', 'status', '--porcelain'], vendor).trim();
  if (dirty) throw new Error('The pinned Unicorn source has local changes.');
  const receipt = {
    unicorn_seconds: unicornSeconds,
    rust_seconds: rustSeconds,
    link_seconds: totalSeconds - unicornSeconds - rustSeconds,
    total_seconds: totalSeconds,
    wasm_bytes: wasm.length,
    wasm_sha256: hash(wasm),
    wasm_gzip_bytes: gzipSync(wasm, { level: 9 }).length,
    loader_bytes: loader.length,
    loader_sha256: hash(loader),
    rust: pinned.nightly,
    emscripten: pinned.emsdk,
    profiling: selected.profiling,
    rust_profile: {
      lto: plan.env.CARGO_PROFILE_RELEASE_LTO ?? 'thin',
      codegen_units: Number(plan.env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS ?? '1'),
      opt_level: plan.env.CARGO_PROFILE_RELEASE_OPT_LEVEL ?? '3',
    },
    rust_flags: plan.rustFlags,
    c_flags: plan.cFlags,
    allocator: selected.allocator,
    simd128: { rust: selected.rustSimd128, c: selected.cSimd128 },
    unicorn_commit: capture(['git', 'rev-parse', 'HEAD'], vendor).trim(),
    unicorn_dirty: false,
    unicorn_tci_sha256: hash(readFileSync(join(vendor, 'qemu/tcg/tci.c'))),
  };
  const receiptPath = join(out, 'build.json');
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  stageRuntimeNotices(out);
  if (selected.stagePackage) stagePackage(out);
  console.log(JSON.stringify(receipt, null, 2));
  return receipt;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.includes('--help') || args.includes('-h')) {
    console.log(
      'Usage: node emulator/build.mts [--out DIR] [--stage-package] [--profiling] [--rust-simd128] [--c-simd128] [--lto thin|fat|off] [--opt-level 0|1|2|3|s|z] [--codegen-units N] [--allocator dlmalloc|emmalloc]',
    );
  } else {
    build(options(args));
  }
}
