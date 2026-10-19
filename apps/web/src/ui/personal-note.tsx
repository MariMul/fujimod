import { For, createSignal, onSettled } from 'solid-js';
import cameraInventory from '../../data/cameras.json';
import featureIcons from './assets/feature-icons.svg?url';
import type { CameraSupport } from './cameras';
const labels: Record<string, string> = {
  aspect: 'Aspect ratio',
  highlight: 'Highlight metering',
  snap: 'Snap focus',
  recipes: 'Save recipe',
  'white-balance': 'White balance',
  shift: 'White balance shift',
  film: 'Film simulation',
  eterna: 'ETERNA',
  'classic-neg': 'Classic Neg.',
  'bleach-bypass': 'Bleach Bypass',
  'nostalgic-neg': 'Nostalgic Neg.',
  'reala-ace': 'REALA ACE',
};

const cameraCount = new Set(cameraInventory.map((camera) => camera.name)).size;
const ratios = ['65-24', '17-6', '3-2', '4-3', '16-9', '3-4', '1-1', '7-6', '5-4'];
const films = ['classic-neg', 'bleach-bypass', 'nostalgic-neg', 'reala-ace'];
export function projectAge(date: Date) {
  const today = Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  return Math.max(0, Math.floor((today - Date.UTC(2026, 9, 3)) / 86_400_000));
}
function FeatureIcon(props: { name: string; ratio?: boolean }) {
  const label = () => (props.ratio ? props.name.replace('-', ':') : labels[props.name]);
  return (
    <svg class={['feature-icon', { ratio: props.ratio }]} role="img" aria-label={label()}>
      <title>{label()}</title>
      <use href={`${featureIcons}#${props.name}`} />
    </svg>
  );
}
function IconList(props: { names: string[]; ratio?: boolean }) {
  return (
    <For each={props.names}>
      {(name, index) => (
        <>
          {index() ? ', ' : ''}
          <FeatureIcon name={name} ratio={props.ratio} />
        </>
      )}
    </For>
  );
}
declare const __FUJIMOD_BUILD_DAY__: string;
export function PersonalNote(props: { cameras: CameraSupport[] }) {
  const [age, setAge] = createSignal(projectAge(new Date(`${__FUJIMOD_BUILD_DAY__}T12:00:00`)));
  onSettled(() => {
    setAge(projectAge(new Date()));
  });
  const supported = () => {
    const models = [
      ...new Set(
        props.cameras.filter((camera) => camera.mod).map((camera) => camera.firmware.model),
      ),
    ];
    return models.length
      ? new Intl.ListFormat('en', { style: 'long', type: 'conjunction' }).format(models)
      : 'no cameras in this build';
  };
  return (
    <aside class="personal-note" aria-label="A note from Jake">
      <p>Hey,</p>
      <p>
        FujiMod is a <span id="project-age">{age()}</span> day old unauthorized firmware
        modification for Fujifilm X-series cameras. It has features like{' '}
        <mark>
          <FeatureIcon name="aspect" /> aspect ratio crop
        </mark>{' '}
        (
        <span id="crop-ratios">
          <IconList names={ratios} ratio />
        </span>
        ),{' '}
        <mark>
          <FeatureIcon name="highlight" /> highlight metering
        </mark>
        ,{' '}
        <mark>
          <FeatureIcon name="snap" /> snap focus
        </mark>
        ,{' '}
        <mark>
          unlimited <FeatureIcon name="recipes" /> recipes
        </mark>{' '}
        (that save <FeatureIcon name="white-balance" /> white balance &amp;{' '}
        <mark>
          <FeatureIcon name="shift" /> shift
        </mark>
        ), more <FeatureIcon name="film" /> film simulations (<FeatureIcon name="eterna" /> ETERNA
        👍,{' '}
        <span id="other-films">
          <IconList names={films} />
        </span>{' '}
        🤷) and{' '}
        <a href="#features">
          more
        </a>
        . Although it currently supports <span id="supported-models">{supported()}</span>, we have a
        plan to eventually support all <span id="camera-count">{cameraCount}</span> camera models.
      </p>
      <p>
        FujiMod is <mark>not ready</mark> for your camera. It is too new and changing too quickly.
        I’ll release <mark>SD card dual boot</mark> and then <mark>installable firmware</mark> as I
        gain confidence in our design and simulation testing system.
      </p>
      <p>
        Until then, you can try it in the <em>camera emulator</em> below.
      </p>
      <p class="signature">-Jake</p>
    </aside>
  );
}
