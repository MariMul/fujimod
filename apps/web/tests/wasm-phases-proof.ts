import { createEngine } from '../src/engine';
import type { CardEntry } from '../src/engine/api';
import { LCDViewer } from '../src/viewer/lcd';

const canvas = document.createElement('canvas'),
  output = document.createElement('pre');
canvas.id = 'lcd-source';
canvas.style.cssText = 'width:720px;height:480px;display:block;image-rendering:pixelated';
document.body.append(canvas, output);
const viewer = new LCDViewer(document.body, canvas);
const result: Record<string, unknown> = { phases: [] };
const phases = result.phases as Record<string, unknown>[];
const errors: string[] = [];
const engine = createEngine((error) => errors.push(String(error)));
window.cameraEngine = engine;
let delivered = 0;
engine.frames((frame) => {
  viewer.frame(frame);
  delivered++;
});
engine.metrics((metrics) => {
  window.cameraPerformance = metrics;
});
const show = () => {
  output.textContent = JSON.stringify(result, null, 2);
};
const check = (value: unknown, message: string) => {
  if (!value) throw Error(message);
};
type Profile = Record<string, unknown> & { ticks: string; observations: Record<string, number[]> };
const profile = async () => (await engine.profile(0)) as Profile;
const u32 = (bytes: number[]) => new DataView(Uint8Array.from(bytes).buffer).getUint32(0, true);
const ready = (p: Profile) =>
  u32(p.observations.mode!) === 3 && !u32(p.observations.pending!) && !u32(p.observations.busy!);
const advance = (ticks = 1_200_000) => engine.replay([{ action: 'advance', ticks }]);
const tap = async (id: string) => {
  await engine.press(id);
  await engine.release(id);
};
const hash = async (bytes: BufferSource) =>
  [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
    .map((n) => n.toString(16).padStart(2, '0'))
    .join('');
const header = () => hash(canvas.getContext('2d')!.getImageData(0, 0, 720, 48).data);
const files = (entries: CardEntry[]): CardEntry[] =>
  entries.flatMap((e) => (e.kind === 'directory' ? files(e.children ?? []) : [e]));
async function until(test: (p: Profile) => boolean | Promise<boolean>, maximum = 120_000_000) {
  const start = BigInt((await profile()).ticks);
  for (;;) {
    const p = await profile();
    if (await test(p)) return p;
    if (BigInt(p.ticks) - start >= BigInt(maximum))
      throw Error('Guest completion deadline exceeded.');
    await advance();
  }
}
async function phase(name: string, operation: () => Promise<void>, cold = false) {
  const before = cold ? undefined : await profile();
  const wall = performance.now();
  let error: string | undefined;
  try {
    await operation();
  } catch (failure) {
    error = String(failure);
  }
  const end = performance.now(),
    after = await profile();
  const guestSeconds = Number(BigInt(after.ticks) - BigInt(before?.ticks ?? '0')) / 12_000_000;
  phases.push({
    name,
    guestSeconds,
    wallSeconds: (end - wall) / 1000,
    speed: guestSeconds / ((end - wall) / 1000),
    completed: !error,
    error,
    before,
    after,
  });
  console.log(
    JSON.stringify({ phase: name, ...phases.at(-1), before: undefined, after: undefined }),
  );
  show();
}
function quality(expected: Uint8ClampedArray, actual: Uint8ClampedArray) {
  const sums = Array.from({ length: 3 }, () => [0, 0, 0, 0, 0, 0]);
  let count = 0;
  for (let y = 64; y < 416; y++)
    for (let x = 24; x < 696; x++) {
      count++;
      for (let c = 0; c < 3; c++) {
        const at = (y * 720 + x) * 4 + c,
          a = expected[at]!,
          b = actual[at]!,
          v = sums[c]!;
        v[0] += Math.abs(a - b);
        v[1] += a;
        v[2] += b;
        v[3] += a * a;
        v[4] += b * b;
        v[5] += a * b;
      }
    }
  return {
    mae: sums.map((v) => v[0]! / count),
    correlation: sums.map(
      (v) =>
        (v[5]! - (v[1]! * v[2]!) / count) /
        Math.sqrt((v[3]! - v[1]! ** 2 / count) * (v[4]! - v[2]! ** 2 / count)),
    ),
  };
}
try {
  // The caller-supplied DAT is selected before power-on. Runtime loading, verification,
  // construction, boot execution, initial checkpoint and first delivered LCD are timed.
  const dat = await (await fetch('/__wasm-camera/firmware.dat')).arrayBuffer();
  const config = {
    firmwareId: 'x70-110',
    bundle: '/__wasm-camera/',
    sceneInput: true,
    execution: { mode: 'replay' as const, clockSeconds2000: 845000000 },
  };
  await phase(
    'cold boot to REC',
    async () => {
      engine.prepare(config);
      await engine.boot(dat, config, () => {});
      await until(ready);
      check(canvas.width === 720 && canvas.height === 480, 'REC LCD was not delivered.');
    },
    true,
  );
  await phase('REC live view', async () => {
    const frames = delivered;
    await advance(120_000_000);
    result.recDeliveredFrames = delivered - frames;
    check(delivered - frames >= 100, 'REC lost LCD presentation frames.');
    check(ready(await profile()), 'REC readiness differs.');
  });
  await phase('MENU navigation', async () => {
    const rec = await header();
    await tap('menu_ok_button');
    await until(async () => (await header()) !== rec, 60_000_000);
    await tap('selector_function_buttons.down');
    await advance(3_600_000);
    await tap('disp_back_button');
    await until(async (p) => ready(p) && (await header()) === rec, 60_000_000);
  });
  // This phase requires the stock AF completion notification or a stable REC21=8.
  await engine.setPosition('focus_mode_selector', 'S');
  await advance(6_000_000);
  await phase('AF-S half-press', async () => {
    let last = u32((await profile()).observations.afUI!),
      changed = false,
      stable: bigint | undefined;
    await engine.press('shutter_button', 'half');
    await until((p) => {
      const notification = u32(p.observations.afUI!);
      changed ||= notification !== last;
      last = notification;
      if (changed && [0x816, 0x829].includes(notification)) return true;
      const machine = new DataView(Uint8Array.from(p.observations.rec21!).buffer).getUint16(
        0,
        true,
      );
      if (machine !== 8) stable = undefined;
      else if (stable === undefined) stable = BigInt(p.ticks);
      return stable !== undefined && BigInt(p.ticks) - stable >= 1_200_000n;
    });
  });
  await engine.release('shutter_button');
  // An incomplete AF phase must not poison the independent capture/Playback measurements.
  await engine.powerCycle([], () => {});
  check(ready(await profile()), 'Capture cold-start prerequisite failed.');
  // Capture is independently qualified with the physical manual-focus position.
  await engine.setPosition('focus_mode_selector', 'M');
  await engine.setPosition('shutter_speed_dial', '125');
  await advance(6_000_000);
  let jpeg: Uint8Array<ArrayBuffer> | undefined;
  await phase('capture to closed JPEG', async () => {
    const seen = new Set(files((await engine.cardTree()).entries).map((f) => f.path));
    await engine.setScene(null);
    await engine.press('shutter_button', 'half');
    await engine.press('shutter_button', 'full');
    await until(async () => {
      if ((await engine.cardState()).busy) return false;
      try {
        const photo = files((await engine.cardTree()).entries).find(
          (f) => /\.JPG$/i.test(f.path) && !seen.has(f.path),
        );
        if (!photo) return false;
        jpeg = (await engine.readCardFile(photo.path)).slice();
        return (
          jpeg.length > 100000 &&
          jpeg[0] === 255 &&
          jpeg[1] === 216 &&
          jpeg.at(-2) === 255 &&
          jpeg.at(-1) === 217
        );
      } catch (error) {
        if (!/busy|chain|FAT|directory|extent|sector|cluster|file|revision/i.test(String(error)))
          throw error;
        return false;
      }
    }, 180_000_000);
    result.jpeg = { bytes: jpeg!.length, sha256: await hash(jpeg!.slice().buffer), focus: 'M' };
  });
  await engine.release('shutter_button');
  await advance(6_000_000);
  await phase('Playback', async () => {
    check(jpeg, 'Capture prerequisite failed.');
    const image = await createImageBitmap(new Blob([jpeg!.slice().buffer]));
    const reference = new OffscreenCanvas(720, 480).getContext('2d')!;
    reference.drawImage(image, 0, 0, 720, 480);
    image.close();
    const expected = reference.getImageData(0, 0, 720, 480).data;
    await tap('playback_button');
    await until((p) => {
      if (u32(p.observations.mode!) !== 4 || u32(p.observations.busy!)) return false;
      const q = quality(expected, canvas.getContext('2d')!.getImageData(0, 0, 720, 480).data);
      result.playbackQuality = q;
      return Math.max(...q.mae) <= 3 && Math.min(...q.correlation) >= 0.98;
    }, 240_000_000);
  });
  const jit = (await profile()).jit as {
    enabled: boolean;
    compiledExecutions: string;
    failures: number;
  };
  result.jit = jit;
  check(
    jit.enabled && BigInt(jit.compiledExecutions) > 0n && jit.failures === 0,
    'Default JIT execution failed.',
  );
  result.errors = errors;
  result.passed =
    phases.length === 6 &&
    phases.every((p) => p.completed && Number(p.speed) >= 1) &&
    !errors.length;
  document.body.dataset.proof = result.passed ? 'passed' : 'failed';
} catch (error) {
  result.error = String(error);
  document.body.dataset.proof = 'failed';
} finally {
  show();
  engine.dispose();
}
