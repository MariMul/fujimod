import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, expect, test } from 'vite-plus/test';
import {
  STAGES,
  byModel,
  phases,
  started,
  status,
  type FirmwareProgress,
  type Stage,
  type StageProgress,
} from './camera-progress';
import { CameraTable } from './cameras';
let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
});
function progress(
  model: string,
  passed: Stage[],
  mods: Record<string, 'pass' | 'fail'> = {},
  version = '1.00',
): FirmwareProgress {
  const stages = Object.fromEntries(
    STAGES.map((stage): [Stage, StageProgress] => [
      stage,
      passed.includes(stage) ? 'pass' : 'not-run',
    ]),
  ) as Record<Stage, StageProgress>;
  return { firmware_id: `${model.toLowerCase()}-100`, model, version, stages, mods };
}
function table(
  columns: Parameters<typeof CameraTable>[0]['columns'],
  documents: FirmwareProgress[],
) {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <CameraTable columns={columns} progress={byModel(documents)} />, host);
  flush();
  return host;
}

test('a stage fills its share of its phase and the row names the first unfinished phase', () => {
  const list = phases(
    progress('X-E2S', ['analysis', 'rec', 'camera-identity', 'mod-target', 'mod-build'], {
      a: 'pass',
      b: 'fail',
      c: 'fail',
      d: 'fail',
    }),
  );
  expect(list.map((phase) => [phase.id, phase.fraction, phase.value])).toEqual([
    ['emulator', 2 / 3, '2 of 3'],
    ['camera', 1 / 3, '1 of 3'],
    ['mods', 1 / 4, '1 of 4'],
    ['sd-card', 0, 'Not yet'],
  ]);
  expect(status(list)).toBe('Emulator');
  const later = phases(
    progress(
      'X70',
      ['analysis', 'rec', 'menu', 'camera-identity', 'camera-snapshot', 'camera-menu'],
      { a: 'pass', b: 'fail' },
    ),
  );
  expect(status(later)).toBe('1 of 2 mods');
});

test('stale and failing stages do not count', () => {
  const document = progress('X-T10', ['analysis']);
  document.stages.rec = 'stale';
  document.stages.menu = 'fail';
  expect(phases(document)[0].fraction).toBe(1 / 3);
});

test('a model with only static analysis is not started', () => {
  expect(started(progress('X30', ['analysis']))).toBe(false);
  expect(started(progress('X30', ['analysis', 'rec']))).toBe(true);
  expect(started(progress('X30', ['analysis'], { a: 'pass' }))).toBe(true);
});

test('the newest firmware version with progress represents its model', () => {
  const documents = byModel([
    progress('X-M1', ['analysis'], {}, '1.50'),
    progress('X-M1', ['analysis'], {}, '1.20'),
    progress('X-M1', ['analysis'], {}, '1.5'),
  ]);
  expect(documents['X-M1']?.version).toBe('1.50');
});

test('started models draw a bar; the others are listed as not started', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '2013–2016', models: ['X70', 'X30', 'X20'] }]],
    [progress('X70', ['analysis', 'rec']), progress('X30', ['analysis'])],
  );
  expect([...host.querySelectorAll('.progress-row .name')].map((row) => row.textContent)).toEqual([
    'X70',
  ]);
  expect(host.querySelector('.rest')?.textContent).toBe('Not startedX30 · X20');
  expect(host.querySelectorAll('.progress-row .phase')).toHaveLength(4);
});

test('the camera popover asks an owner for help until every camera step passes', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '', models: ['X-E2', 'X-E2S', 'X70'] }]],
    [
      progress('X-E2', ['analysis', 'rec']),
      progress('X-E2S', ['analysis', 'rec', 'camera-identity']),
      progress('X70', ['analysis', 'rec', 'camera-identity', 'camera-snapshot', 'camera-menu']),
    ],
  );
  const help = [...host.querySelectorAll('.bar')].map(
    (bar) => bar.querySelector('.phase:nth-child(2) .help')?.textContent,
  );
  expect(help).toEqual(['Have an X-E2? Help test it', 'Help test on your X-E2S', undefined]);
  expect(host.querySelector('.phase:nth-child(2) .tip-value')?.textContent).toBe('Needs an owner');
});

test('a started model missing from the generation list appears under Other cameras', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '', models: ['X70'] }]],
    [progress('X-A2', ['analysis', 'rec'])],
  );
  const groups = [...host.querySelectorAll('.gen h4')].map((heading) => heading.textContent);
  expect(groups).toEqual(['EXR Processor II', 'Other cameras']);
  expect(host.querySelector('.progress-row .name')?.textContent).toBe('X-A2');
});
