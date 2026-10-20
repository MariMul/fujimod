import { createEngine } from '../src/engine';
import type { EngineConfig } from '../src/engine/api';
import type { EngineFault } from '../src/engine/fault';
import { LCDViewer } from '../src/viewer/lcd';

const host = document.createElement('main');
const canvas = document.createElement('canvas');
canvas.style.cssText = 'width:320px;height:320px;image-rendering:pixelated;display:block';
const result = document.createElement('pre');
host.append(canvas, result);
document.body.append(host);
const viewer = new LCDViewer(host, canvas);
const errors: string[] = [],
  faults: EngineFault[] = [],
  frames: { sequence: number; pixels: number[] }[] = [];
const config = (fault = false): EngineConfig => ({
  firmwareId: 'generated-arm',
  bundle: `/runtime/${fault ? 'wasm-proof-fault' : 'wasm-proof'}/`,
  inputs: '/runtime/wasm-proof/inputs.json',
  execution: { mode: 'replay', clockSeconds2000: 0 },
  boot: {
    datPath: '',
    prepare: { module: '', function: '', options: {} },
    module: '',
    sessionModule: '',
    limit: 0,
    step: 0,
    readyLimit: 0,
    readyWhen: [],
    sceneInput: false,
  },
});
function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw Error(message);
}
const engine = createEngine({ ...config(), adapter: 'wasm' }, (error) =>
  errors.push(String(error)),
);
engine.frames((frame) => {
  viewer.frame(frame);
  frames.push({
    sequence: frame.sequence,
    pixels: [...canvas.getContext('2d')!.getImageData(0, 0, 2, 2).data],
  });
});
let metrics: unknown;
engine.metrics((value) => {
  metrics = value;
});
try {
  const response = await fetch('/runtime/wasm-proof/program.dat');
  assert(response.ok, 'Prepare the Wasm proof artifacts first.');
  const dat = await response.arrayBuffer();
  const started = performance.now();
  const inputs = await engine.boot(dat.slice(0), config(), (message) => {
    result.textContent = message;
  });
  const bootMilliseconds = performance.now() - started;
  assert(inputs[0]?.id === 'MENU', 'Physical input metadata differs.');
  await engine.replay([
    { action: 'advance', ticks: 2_000_000 },
    { action: 'press', id: 'MENU' },
    { action: 'advance', ticks: 2_500_000 },
  ]);
  assert(
    JSON.stringify(frames.at(-1)?.pixels) === JSON.stringify(Array(4).fill([0, 1, 35, 255]).flat()),
    'Pressed frame differs.',
  );
  await engine.replay([
    { action: 'advance', ticks: 500_000 },
    { action: 'release', id: 'MENU' },
    { action: 'advance', ticks: 2_500_000 },
  ]);
  assert(
    JSON.stringify(frames.at(-1)?.pixels) === JSON.stringify(Array(4).fill([0, 4, 86, 255]).flat()),
    'Released frame differs.',
  );
  assert(errors.length === 0, 'Unexpected execution fault.');
  const missing = await engine.cardState().then(
    () => '',
    (error) => String(error),
  );
  assert(missing.includes('camera_command'), 'Missing session ABI must reject card operations.');
  const bad = createEngine({ ...config(), adapter: 'wasm' }, () => {});
  try {
    const changed = dat.slice(0);
    new Uint8Array(changed)[0] ^= 1;
    const rejection = await bad
      .boot(changed, config(), () => {})
      .then(
        () => '',
        (error) => JSON.stringify(error.fault),
      );
    assert(rejection.includes('SHA-256'), 'Module did not reject modified DAT.');
  } finally {
    bad.dispose();
  }
  const failing = createEngine({ ...config(), adapter: 'wasm' }, () => {});
  failing.errors((fault) => {
    faults.push(fault);
  });
  try {
    const bytes = await (await fetch('/runtime/wasm-proof-fault/program.dat')).arrayBuffer();
    await failing.boot(bytes, config(true), () => {});
    await failing.replay([{ action: 'advance', ticks: 100_000 }]).then(
      () => {
        throw Error('Unmapped load accepted.');
      },
      () => {},
    );
    assert(faults.at(-1)?.kind === 'guest-fault', 'Guest fault was not published.');
  } finally {
    failing.dispose();
  }
  result.textContent = JSON.stringify(
    { passed: true, bootMilliseconds, frames, metrics, faults, missing },
    null,
    2,
  );
  document.body.dataset.proof = 'passed';
} catch (error) {
  result.textContent = String(error);
  document.body.dataset.proof = 'failed';
  throw error;
} finally {
  engine.dispose();
}
