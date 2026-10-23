import type { ControlInput } from 'fujimod/emulator';
import { DEVICE_PANEL_SCHEMA, type ControlNode, type DevicePanel } from './layout';

export type Variant = NonNullable<ControlNode['variant']>;

/** How a control is labeled on the panel: face text or an icon, and an optional caption. */
export interface ControlFace {
  face?: string[];
  icon?: string;
  caption?: string;
  record?: boolean;
}

export type InputGroup =
  | {
      kind: 'pad';
      id: string;
      label: string;
      up: ControlInput;
      down: ControlInput;
      left: ControlInput;
      right: ControlInput;
      center?: ControlInput;
    }
  | {
      kind: 'command';
      id: string;
      label: string;
      decrease: ControlInput;
      increase: ControlInput;
      press?: ControlInput;
    }
  | { kind: 'single'; input: ControlInput };

/** Short captions for inputs shared across bodies; other inputs use their label. */
const captions: Record<string, string> = {
  exposure_compensation_dial: 'EV',
  shutter_speed_dial: 'SPEED',
  aperture_ring: 'APERTURE',
  focus_mode_selector: 'FOCUS',
  power_switch: 'POWER',
  on_off_switch: 'POWER',
  mode_dial: 'MODE',
  drive_dial: 'DRIVE',
  lcd_tilt_switch: 'SCREEN',
  auto_mode_selector_lever: 'AUTO',
  control_ring: 'LENS',
  control_ring_options_button: 'SIDE',
  shutter_button: 'SHUTTER',
  command_control: 'CMD',
  main_command_dial: 'MAIN',
  sub_command_dial: 'SUB',
  front_command_dial: 'FRONT',
  rear_command_dial: 'REAR',
  command_dial: 'DIAL',
  zoom_control: 'ZOOM',
};

/** Single-character manual names that stand for a printed icon. */
const glyphs: Record<string, ControlFace> = {
  a: { icon: 'play' },
  '▶': { icon: 'play' },
  b: { icon: 'trash' },
  t: { record: true },
  '●': { record: true },
  N: { icon: 'flash' },
};

const MAX_FACE = 6;
const MAX_CAPTION = 10;

function caption(id: string, name: string): string {
  const known = captions[id];
  if (known) return known;
  const upper = name.toUpperCase();
  return upper.length <= MAX_CAPTION ? upper : upper.split(/\s+/)[0]!.slice(0, MAX_CAPTION);
}

/** The presentations an input allows. Relative motion, positions and stages decide before kind. */
export function compatibleVariants(input: ControlInput): Variant[] {
  if (input.relative) return ['ticks', 'dial'];
  const positions = input.positions?.length ?? 0;
  if (positions > 5) return ['dial', 'segment', 'ticks'];
  if (positions > 0) return ['segment', 'dial', 'ticks'];
  const stages = input.stages?.map((stage) => stage.id) ?? [];
  if (stages.includes('half') && stages.includes('full')) return ['shutter'];
  return ['key', 'record'];
}

/** The face, icon or caption a control shows, derived from its owner's-manual label. */
export function controlFace(input: ControlInput): ControlFace {
  let name = input.label;
  // An ungrouped id may list an alias after " / "; a grouped id names its group first.
  if (name.includes(' / ')) name = name.split(' / ')[input.id.includes('.') ? 1 : 0]!;
  name = name.replace(/\s+(button|switch|Switch)$/, '');
  if (/^movie[- ]record/i.test(name)) return { record: true };
  const meaning = /^(.*?)\s*\(([^)]*)\)(.*)$/.exec(name);
  if (meaning) {
    const outer = meaning[1]!.replace(/\s+button$/, '');
    name = outer === 'Function' ? meaning[2]! : `${outer}${meaning[3]}`.trim();
  }
  const glyph = glyphs[name];
  if (glyph) return glyph;
  const lines = name.split('/');
  if (lines.every((line) => /^[A-Z0-9][A-Za-z0-9-]*$/.test(line) && line.length <= MAX_FACE))
    return { face: lines };
  return { caption: caption(input.id.split('.')[0]!, name) };
}

/** Group pad directions and command steps by the id prefix before ".". */
export function groupInputs(inputs: readonly ControlInput[]): InputGroup[] {
  const byPrefix = new Map<string, Map<string, ControlInput>>();
  for (const input of inputs) {
    const dot = input.id.indexOf('.');
    if (dot < 0) continue;
    const prefix = input.id.slice(0, dot);
    const members = byPrefix.get(prefix) ?? new Map<string, ControlInput>();
    members.set(input.id.slice(dot + 1), input);
    byPrefix.set(prefix, members);
  }
  const menuOk = inputs.find((input) => input.id === 'menu_ok_button');
  const grouped = new Set<string>();
  const groups = new Map<string, InputGroup>();
  for (const [prefix, members] of byPrefix) {
    const up = members.get('up'),
      down = members.get('down'),
      left = members.get('left'),
      right = members.get('right');
    const label = [...members.values()][0]!.label.split(' / ')[0]!;
    if (up && down && left && right) {
      const center = members.get('press') ?? menuOk;
      groups.set(prefix, { kind: 'pad', id: prefix, label, up, down, left, right, center });
      for (const input of [up, down, left, right, center]) if (input) grouped.add(input.id);
      continue;
    }
    const decrease = members.get('decrease') ?? members.get('wide');
    const increase = members.get('increase') ?? members.get('tele');
    if (decrease && increase) {
      const press = members.get('press');
      groups.set(prefix, { kind: 'command', id: prefix, label, decrease, increase, press });
      for (const input of [decrease, increase, press]) if (input) grouped.add(input.id);
    }
  }
  const result: InputGroup[] = [];
  for (const input of inputs) {
    const prefix = input.id.split('.')[0]!;
    const group = groups.get(prefix);
    if (group) {
      if (!result.includes(group)) result.push(group);
    } else if (!grouped.has(input.id)) result.push({ kind: 'single', input });
  }
  return result;
}

function shown(input: ControlInput) {
  return input.supported && input.kind !== 'touch' && !input.id.endsWith('_latch');
}

/** Keys beside the D-pad and command controls, as on the back of the camera. */
const SIDE_KEYS = 6;

function controlNode(input: ControlInput): ControlNode {
  const variant = compatibleVariants(input)[0]!;
  const node: ControlNode = { kind: 'control', id: input.id, variant };
  if (variant !== 'key' && variant !== 'record') {
    node.caption = caption(
      input.id,
      input.label.replace(/\s+(dial|ring|switch|selector|lever|button)$/i, ''),
    );
    if (input.positions)
      node.labels = Object.fromEntries(
        input.positions.map((position) => [position.id, position.label]),
      );
    return node;
  }
  const face = controlFace(input);
  if (face.record) return { ...node, variant: 'record' };
  if (face.icon) node.icon = face.icon;
  if (face.face) {
    node.face = face.face;
    if (face.face.some((line) => line.length > 3)) node.className = 'sm';
  }
  if (face.caption) node.caption = face.caption;
  return node;
}

function groupNode(group: Exclude<InputGroup, { kind: 'single' }>): ControlNode {
  if (group.kind === 'pad') {
    const center = group.center;
    const face = center?.id.endsWith('.press')
      ? ['OK']
      : center
        ? (controlFace(center).face ?? [])
        : [];
    return {
      kind: 'dpad',
      face,
      bindings: {
        up: group.up.id,
        down: group.down.id,
        left: group.left.id,
        right: group.right.id,
        ...(center ? { center: center.id } : {}),
      },
    };
  }
  return {
    kind: 'command',
    caption: caption(group.id, group.label),
    bindings: {
      left: group.decrease.id,
      right: group.increase.id,
      ...(group.press ? { center: group.press.id } : {}),
    },
  };
}

/** A usable panel for any body: supported controls, grouped, with short faces. */
export function defaultLayout(inputs: readonly ControlInput[], body: string): DevicePanel {
  const groups = groupInputs(inputs.filter(shown));
  const positional: ControlNode[] = [];
  const clusters: ControlNode[] = [];
  const keys: ControlNode[] = [];
  for (const group of groups) {
    if (group.kind !== 'single') clusters.push(groupNode(group));
    else if (compatibleVariants(group.input)[0] === 'key') keys.push(controlNode(group.input));
    else positional.push(controlNode(group.input));
  }
  positional.sort((a, b) => Number(b.variant === 'shutter') - Number(a.variant === 'shutter'));
  const layout: ControlNode[] = [];
  if (positional.length) layout.push({ kind: 'grid', className: 'g2', children: positional });
  if (clusters.length || keys.length) {
    if (layout.length) layout.push({ kind: 'divider' });
    const rest = clusters.length ? keys.slice(SIDE_KEYS) : keys;
    if (clusters.length)
      layout.push({
        kind: 'grid',
        className: 'back',
        children: [
          {
            kind: 'column',
            className: 'c1',
            children: [
              { kind: 'grid', className: 'side-keys', children: keys.slice(0, SIDE_KEYS) },
            ],
          },
          { kind: 'column', className: 'c2 clusters', children: clusters },
        ],
      });
    if (rest.length) layout.push({ kind: 'grid', className: 'key-grid', children: rest });
  }
  return { schema: DEVICE_PANEL_SCHEMA, body, combos: [], layout };
}
