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
import { CameraTable, type Owner } from './cameras';
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
  owners: Owner[] = [],
) {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => <CameraTable columns={columns} progress={byModel(documents)} owners={owners} />,
    host,
  );
  flush();
  return host;
}

test('a stage fills its share of its phase and the row names the first unfinished phase', () => {
  const list = phases(
    progress('X-E2S', ['research', 'rec', 'camera-identity', 'mod-target', 'mod-build'], {
      a: 'pass',
      b: 'fail',
      c: 'fail',
      d: 'fail',
    }),
  );
  expect(list.map((phase) => [phase.id, phase.fraction, phase.value])).toEqual([
    ['research', 2 / 3, '2 of 3'],
    ['emulator', 1 / 2, '1 of 2'],
    ['camera', 1 / 3, '1 of 3'],
    ['mods', 1 / 4, '1 of 4'],
    ['sd-card', 0, 'Not yet'],
  ]);
  expect(status(list)).toBe('Emulator');
  expect(status(phases(progress('X-T4', ['research'])))).toBe('Research');
  const later = phases(
    progress(
      'X70',
      ['research', 'analysis', 'rec', 'menu', 'camera-identity', 'camera-snapshot', 'camera-menu'],
      { a: 'pass', b: 'fail' },
    ),
  );
  expect(status(later)).toBe('1 of 2 mods');
});

test('stale and failing stages do not count', () => {
  const document = progress('X-T10', ['rec']);
  document.stages.menu = 'stale';
  document.stages.research = 'fail';
  const [research, emulator] = phases(document);
  expect([research?.fraction, emulator?.fraction]).toEqual([0, 1 / 2]);
});

test('research steps: unpacking and export share one stage, boot facts the other', () => {
  const steps = (passed: Stage[]) =>
    phases(progress('X30', passed))[0]?.steps.map((step) => [step.label, step.done]);
  expect(steps(['research'])).toEqual([
    ['Firmware unpacked', true],
    ['Code exported', true],
    ['Boot facts located', false],
  ]);
  expect(steps(['analysis'])?.map(([, done]) => done)).toEqual([false, false, true]);
});

test('a model is started once any stage or mod passes', () => {
  expect(started(progress('X30', []))).toBe(false);
  expect(started(progress('X30', [], { a: 'fail' }))).toBe(false);
  expect(started(progress('X30', [], { a: 'pass' }))).toBe(true);
  expect(started(progress('X30', ['camera-snapshot']))).toBe(true);
  expect(started(progress('X30', ['research']))).toBe(true);
  expect(started(progress('X30', ['analysis']))).toBe(true);
});

test('the newest firmware version with progress represents its model', () => {
  const documents = byModel([
    progress('X-M1', ['research'], {}, '1.50'),
    progress('X-M1', ['research'], {}, '1.20'),
    progress('X-M1', ['research'], {}, '1.5'),
  ]);
  expect(documents['X-M1']?.version).toBe('1.50');
});

test('started models draw a bar; the others are listed as not started', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '2013–2016', models: ['X70', 'X30', 'X20'] }]],
    [progress('X70', ['research', 'rec']), progress('X30', [], { a: 'fail' })],
  );
  expect([...host.querySelectorAll('.progress-row .name')].map((row) => row.textContent)).toEqual([
    'X70',
  ]);
  expect(host.querySelector('.rest')?.textContent).toBe('Not startedX30 · X20');
  expect(host.querySelectorAll('.progress-row .phase')).toHaveLength(5);
});

test('research fills a third for each of its steps', () => {
  expect(phases(progress('X70', ['research', 'analysis']))[0]?.fraction).toBe(1);
  const [research] = phases(progress('X30', ['research']));
  expect([research?.fraction, research?.value]).toEqual([2 / 3, '2 of 3']);
  expect(phases(progress('X20', ['analysis']))[0]?.fraction).toBe(1 / 3);
});

test('one tip is open at a time, whether tapped, hovered or focused', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '', models: ['X70', 'X30'] }]],
    [progress('X70', ['research']), progress('X30', ['research'])],
  );
  const [a, b] = [...host.querySelectorAll<HTMLElement>('.bar')].map((bar) =>
    bar.querySelector<HTMLElement>('.phase:nth-child(2)')!,
  );
  const open = () => [...host.querySelectorAll('.phase.open')];
  const tap = (element: Element) => {
    element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerType: 'touch' }));
    element.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    flush();
  };
  tap(a!);
  expect(open()).toEqual([a]);
  tap(b!);
  expect(open()).toEqual([b]);
  tap(b!);
  expect(open()).toEqual([]);
  a!.dispatchEvent(new PointerEvent('pointerenter', { pointerType: 'mouse' }));
  flush();
  expect(open()).toEqual([a]);
  b!.focus();
  flush();
  expect(open()).toEqual([b]);
  document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
  flush();
  expect(open()).toEqual([]);
});

test('the camera popover asks an owner for help until every camera step passes', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '', models: ['X-E2', 'X-E2S', 'X70'] }]],
    [
      progress('X-E2', ['research', 'rec']),
      progress('X-E2S', ['research', 'rec', 'camera-identity']),
      progress('X70', ['research', 'rec', 'camera-identity', 'camera-snapshot', 'camera-menu']),
    ],
  );
  const help = [...host.querySelectorAll('.bar')].map(
    (bar) => bar.querySelector('.phase:nth-child(3) .help')?.textContent,
  );
  expect(help).toEqual(['Have an X-E2? Help test it', 'Help test on your X-E2S', undefined]);
  expect(host.querySelector('.phase:nth-child(3) .tip-value')?.textContent).toBe('Needs an owner');
});

test('a started model missing from the generation list appears under Other cameras', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '', models: ['X70'] }]],
    [progress('X-A2', ['research', 'rec'])],
  );
  const groups = [...host.querySelectorAll('.gen h4')].map((heading) => heading.textContent);
  expect(groups).toEqual(['EXR Processor II', 'Other cameras']);
  expect(host.querySelector('.progress-row .name')?.textContent).toBe('X-A2');
});

test('bodies a contributor owns are outlined and the camera popover names the owners', () => {
  const host = table(
    [[{ name: 'EXR Processor II', years: '', models: ['X70', 'X30', 'X100V', 'X20'] }]],
    [progress('X70', ['research']), progress('X30', ['research'])],
    [
      { name: 'One', github: 'one', bodies: ['X70', 'X100V'] },
      { name: 'Two', github: 'two', bodies: ['X70'] },
    ],
  );
  const rows = [...host.querySelectorAll('.progress-row')].map((row) => [
    row.querySelector('.name')?.textContent,
    row.classList.contains('owned'),
  ]);
  expect(rows).toEqual([
    ['X70', true],
    ['X30', false],
  ]);
  expect([...host.querySelectorAll('.rest .owned')].map((name) => name.textContent)).toEqual([
    'X100V',
  ]);
  expect(host.querySelector('.rest')?.textContent).toBe('Not startedX100V · X20');
  expect(host.querySelector('.legend .owned')?.textContent).toBe('Owned by a contributor');
  const [owned, unowned] = [...host.querySelectorAll('.bar .phase:nth-child(3)')];
  expect(owned?.querySelector('.tip-value')?.textContent).toBe('Not yet');
  expect(
    [...(owned?.querySelectorAll('.owners a') ?? [])].map((link) => [
      link.textContent,
      link.getAttribute('href'),
    ]),
  ).toEqual([
    ['@one', 'https://github.com/one'],
    ['@two', 'https://github.com/two'],
  ]);
  expect(owned?.querySelector('.help')?.textContent).toBe('Have an X70? Help test it');
  expect(unowned?.querySelector('.tip-value')?.textContent).toBe('Needs an owner');
  expect(unowned?.querySelector('.owners')).toBeNull();
});
