/** Bring-up progress per camera model, from the catalog's generated progress documents. */
export type StageProgress = 'pass' | 'fail' | 'stale' | 'not-run';
export type ModProgress = StageProgress | 'unavailable';
export const STAGES = [
  'research',
  'analysis',
  'rec',
  'menu',
  'camera-identity',
  'camera-snapshot',
  'camera-menu',
  'mod-target',
  'mod-build',
  'sd-image',
] as const;
export type Stage = (typeof STAGES)[number];
export interface FirmwareProgress {
  firmware_id: string;
  model: string;
  version: string;
  stages: Record<Stage, StageProgress>;
  mods: Record<string, ModProgress>;
}

export interface Step {
  label: string;
  done: boolean;
}
export interface Phase {
  id: 'research' | 'emulator' | 'camera' | 'mods' | 'sd-card';
  label: string;
  /** Share of the bar's width. */
  weight: number;
  /** Completed share of this phase, 0–1. */
  fraction: number;
  /** "Done", "2 of 3", "6 of 24", "Not yet". */
  value: string;
  steps: Step[];
}

const STEP_LABELS: Record<Exclude<Stage, 'research'>, string> = {
  analysis: 'Boot facts located',
  rec: 'Reaches the shooting screen',
  menu: 'Menus respond',
  'camera-identity': 'Identified over USB',
  'camera-snapshot': 'Memory snapshot',
  'camera-menu': 'Menus driven over USB',
  'mod-target': 'Firmware locations found',
  'mod-build': 'Built for this firmware',
  'sd-image': 'Dual-boot SD card image',
};
function steps(progress: FirmwareProgress, stages: readonly Exclude<Stage, 'research'>[]): Step[] {
  return stages.map((stage) => ({
    label: STEP_LABELS[stage],
    done: progress.stages[stage] === 'pass',
  }));
}
function stepValue(list: Step[]) {
  const done = list.filter((step) => step.done).length;
  return done === list.length ? 'Done' : done ? `${done} of ${list.length}` : 'Not yet';
}
function stepPhase(
  id: Phase['id'],
  label: string,
  weight: number,
  progress: FirmwareProgress,
  stages: readonly Exclude<Stage, 'research'>[],
): Phase {
  const list = steps(progress, stages);
  return {
    id,
    label,
    weight,
    fraction: list.filter((step) => step.done).length / list.length,
    value: stepValue(list),
    steps: list,
  };
}
/** Research: the `research` stage unpacks and exports the firmware; `analysis` locates its boot facts. */
function researchPhase(progress: FirmwareProgress): Phase {
  const exported = progress.stages.research === 'pass',
    list = [
      { label: 'Firmware unpacked', done: exported },
      { label: 'Code exported', done: exported },
      { label: 'Boot facts located', done: progress.stages.analysis === 'pass' },
    ];
  return {
    id: 'research',
    label: 'Research',
    weight: 10,
    fraction: list.filter((step) => step.done).length / list.length,
    value: stepValue(list),
    steps: list,
  };
}

export function phases(progress: FirmwareProgress): Phase[] {
  const mods = Object.values(progress.mods),
    working = mods.filter((result) => result === 'pass').length;
  return [
    researchPhase(progress),
    stepPhase('emulator', 'Emulator', 30, progress, ['rec', 'menu']),
    stepPhase('camera', 'Camera', 20, progress, [
      'camera-identity',
      'camera-snapshot',
      'camera-menu',
    ]),
    {
      id: 'mods',
      label: 'Mods',
      weight: 30,
      fraction: mods.length ? working / mods.length : 0,
      value: working ? `${working} of ${mods.length}` : 'Not yet',
      steps: steps(progress, ['mod-target', 'mod-build']),
    },
    stepPhase('sd-card', 'SD card', 10, progress, ['sd-image']),
  ];
}

/** The first unfinished phase, as the row's status. */
/** The first unfinished phase; research counts only until later work has progress. */
export function status(list: Phase[]): string {
  const underway = list.some((phase) => phase.id !== 'research' && phase.fraction > 0);
  const next = list.find((phase) => phase.fraction < 1 && !(underway && phase.id === 'research'));
  if (!next) return 'Ready';
  if (next.id === 'mods') return next.value === 'Not yet' ? 'Mods' : `${next.value} mods`;
  return next.label;
}

/** Whether any stage or mod passes; otherwise the model is listed as not started. */
export function started(progress: FirmwareProgress): boolean {
  return (
    STAGES.some((stage) => progress.stages[stage] === 'pass') ||
    Object.values(progress.mods).includes('pass')
  );
}

function versionKey(version: string) {
  return version.split('.').map(Number);
}
function newer(a: string, b: string) {
  const [x, y] = [versionKey(a), versionKey(b)];
  for (let index = 0; index < Math.max(x.length, y.length); index++)
    if ((x[index] ?? 0) !== (y[index] ?? 0)) return (x[index] ?? 0) > (y[index] ?? 0);
  return false;
}
/** One document per model: the newest firmware version with recorded progress. */
export function byModel(documents: FirmwareProgress[]): Record<string, FirmwareProgress> {
  const result: Record<string, FirmwareProgress> = {};
  for (const document of documents) {
    const known = result[document.model];
    if (!known || newer(document.version, known.version)) result[document.model] = document;
  }
  return result;
}
