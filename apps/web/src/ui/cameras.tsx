import { For, Show } from 'solid-js';
import { phases, started, status, type FirmwareProgress, type Phase } from './camera-progress';
import './cameras.css';
export interface Generation {
  name: string;
  years: string;
  models: string[];
}
/** Placeholder until each camera has its own page. */
const CONTRIBUTE = 'dev-tools/';
/** Letters whose names start with a vowel sound take "an": an X-E2, an F… */
const AN = /^[AEFHILMNORSX]/;
function help(model: string, phase: Phase) {
  if (phase.id !== 'camera' || phase.fraction === 1) return undefined;
  if (phase.fraction) return `Help test on your ${model}`;
  return `Have ${AN.test(model) ? 'an' : 'a'} ${model}? Help test it`;
}
function Segment(props: { model: string; phase: Phase }) {
  return (
    <li
      class="phase"
      tabindex="0"
      style={{
        '--w': String(props.phase.weight),
        '--p': `${props.phase.fraction * 100}%`,
        '--c': `var(--phase-${props.phase.id})`,
      }}
      aria-label={`${props.phase.label}: ${props.phase.value}`}
    >
      <div class="track">
        <div class="fill" />
      </div>
      <div class="tip">
        <div class="tip-label">{props.phase.label}</div>
        <div class="tip-value">
          {props.phase.id === 'camera' && !props.phase.fraction
            ? 'Needs an owner'
            : props.phase.value}
        </div>
        <ul>
          <For each={props.phase.steps}>
            {(step) => <li class={{ todo: !step.done }}>{step.label}</li>}
          </For>
        </ul>
        <Show when={help(props.model, props.phase)}>
          {(text) => (
            <a class="help" href={CONTRIBUTE}>
              {text()}
            </a>
          )}
        </Show>
      </div>
    </li>
  );
}
function ProgressRow(props: { progress: FirmwareProgress }) {
  const list = () => phases(props.progress);
  return (
    <div class="progress-row">
      <div class="progress-heading">
        <span class="name">{props.progress.model}</span>
        <span class="version">{props.progress.version}</span>
        <span class="detail">{status(list())}</span>
      </div>
      <ul class="bar" aria-label={`${props.progress.model} progress`}>
        <For each={list()}>{(phase) => <Segment model={props.progress.model} phase={phase} />}</For>
      </ul>
    </div>
  );
}
function GenerationGroup(props: { group: Generation; progress: Record<string, FirmwareProgress> }) {
  const rows = () =>
    props.group.models.flatMap((model) => {
      const progress = props.progress[model];
      return progress && started(progress) ? [progress] : [];
    });
  const rest = () =>
    props.group.models.filter((model) => !rows().some((row) => row.model === model));
  return (
    <div class="gen">
      <h4>
        {props.group.name}
        <span>{props.group.years}</span>
      </h4>
      <For each={rows()}>{(progress) => <ProgressRow progress={progress} />}</For>
      <Show when={rest().length}>
        <div class="rest">
          <b>Not started</b>
          {rest().join(' · ')}
        </div>
      </Show>
    </div>
  );
}
const LEGEND: Phase['id'][] = ['emulator', 'camera', 'mods', 'sd-card'];
const LEGEND_LABELS: Record<Phase['id'], string> = {
  emulator: 'Emulator',
  camera: 'Camera',
  mods: 'Mods',
  'sd-card': 'SD card',
};
export function CameraTable(props: {
  columns: Generation[][];
  progress: Record<string, FirmwareProgress>;
}) {
  const listed = () => new Set(props.columns.flat().flatMap((group) => group.models));
  const unlisted = () =>
    Object.values(props.progress)
      .filter((progress) => !listed().has(progress.model) && started(progress))
      .map((progress) => progress.model)
      .sort();
  return (
    <section class="gens" id="camera-table" aria-labelledby="cameras-heading">
      <div class="cameras-head">
        <h2 class="section-heading" id="cameras-heading">
          Cameras
        </h2>
        <ul class="legend" aria-label="Phases">
          <For each={LEGEND}>
            {(id) => <li style={{ '--c': `var(--phase-${id})` }}>{LEGEND_LABELS[id]}</li>}
          </For>
        </ul>
      </div>
      <For each={props.columns}>
        {(groups) => (
          <div class="col">
            <For each={groups}>
              {(group) => <GenerationGroup group={group} progress={props.progress} />}
            </For>
          </div>
        )}
      </For>
      <Show when={unlisted().length}>
        <div class="col">
          <GenerationGroup
            group={{ name: 'Other cameras', years: '', models: unlisted() }}
            progress={props.progress}
          />
        </div>
      </Show>
    </section>
  );
}
