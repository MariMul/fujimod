import type { ControlInput } from 'fujimod/emulator';
import { expect, test } from 'vite-plus/test';
import { compatibleVariants, controlFace, defaultLayout } from './default-layout';
import type { ControlNode } from './layout';
import catalog from './fixtures/catalog-inputs.json';

const bodies = catalog as Record<string, ControlInput[]>;

function nodes(layout: ControlNode[]): ControlNode[] {
  return layout.flatMap((node) => [node, ...nodes(node.children ?? [])]);
}

function boundIds(layout: ControlNode[]): string[] {
  return nodes(layout).flatMap((node) => [
    ...(node.id ? [node.id] : []),
    ...Object.values(node.bindings ?? {}),
  ]);
}

test('an empty input list gives an empty panel', () => {
  expect(defaultLayout([], 'x70')).toEqual({ schema: 1, body: 'x70', combos: [], layout: [] });
});

for (const [body, inputs] of Object.entries(bodies)) {
  test(`${body}: only supported controls, each once, with short faces`, () => {
    const panel = defaultLayout(inputs, body);
    const ids = boundIds(panel.layout);
    const byId = new Map(inputs.map((input) => [input.id, input]));
    for (const id of ids) {
      const input = byId.get(id)!;
      expect(input, id).toBeTruthy();
      expect(input.supported, id).toBe(true);
      expect(input.kind, id).not.toBe('touch');
      expect(id.endsWith('_latch'), id).toBe(false);
    }
    expect(new Set(ids).size).toBe(ids.length);
    const shown = inputs.filter(
      (input) => input.supported && input.kind !== 'touch' && !input.id.endsWith('_latch'),
    );
    expect(ids.sort()).toEqual(shown.map((input) => input.id).sort());
    for (const node of nodes(panel.layout)) {
      for (const line of node.face ?? []) expect(line.length, line).toBeLessThanOrEqual(6);
      expect((node.face ?? []).length).toBeLessThanOrEqual(2);
      expect((node.caption ?? '').length, node.caption).toBeLessThanOrEqual(10);
    }
  });
}

test('direction groups become one D-pad, centered on press or MENU/OK', () => {
  const x20 = nodes(defaultLayout(bodies['x20-103']!, 'x20').layout).filter(
    (n) => n.kind === 'dpad',
  );
  expect(x20).toHaveLength(1);
  expect(x20[0]!.bindings!.center).toBe('selector_button.press');
  const x70 = nodes(defaultLayout(bodies['x70-110']!, 'x70').layout).filter(
    (n) => n.kind === 'dpad',
  );
  expect(x70).toHaveLength(1);
  expect(x70[0]!.bindings!.center).toBe('menu_ok_button');
  expect(x70[0]!.face).toEqual(['MENU', 'OK']);
});

test('step groups become command controls', () => {
  const x70 = nodes(defaultLayout(bodies['x70-110']!, 'x70').layout).find(
    (n) => n.kind === 'command',
  );
  expect(x70!.bindings).toEqual({
    left: 'command_control.decrease',
    right: 'command_control.increase',
    center: 'command_control.press',
  });
  expect(x70!.caption).toBe('CMD');
  const xq1 = nodes(defaultLayout(bodies['xq1-121']!, 'xq1').layout).find(
    (n) => n.kind === 'command',
  );
  expect(xq1!.bindings).toEqual({ left: 'zoom_control.wide', right: 'zoom_control.tele' });
});

test('manual names become faces, icons and captions', () => {
  const x70 = new Map(bodies['x70-110']!.map((input) => [input.id, controlFace(input)]));
  expect(x70.get('playback_button')).toEqual({ icon: 'play' });
  expect(x70.get('delete_button')).toEqual({ icon: 'trash' });
  expect(x70.get('movie_record_button')).toEqual({ record: true });
  expect(x70.get('disp_back_button')).toEqual({ face: ['DISP', 'BACK'] });
  expect(x70.get('menu_ok_button')).toEqual({ face: ['MENU', 'OK'] });
  expect(x70.get('af_ae_lock_button')).toEqual({ face: ['AF-L', 'AE-L'] });
  expect(x70.get('quick_menu_button')).toEqual({ face: ['Q'] });
  expect(x70.get('control_ring_options_button')).toEqual({ caption: 'SIDE' });
  const xe2 = new Map(bodies['xe2-411']!.map((input) => [input.id, controlFace(input)]));
  expect(xe2.get('fn7_button')).toEqual({ face: ['Fn7'] });
  const xq1 = new Map(bodies['xq1-121']!.map((input) => [input.id, controlFace(input)]));
  expect(xq1.get('movie_recording_button')).toEqual({ record: true });
});

test('relative motion, positions and stages decide presentation before kind', () => {
  const dial = bodies['xe2s-111']!.find((input) => input.id === 'command_dial')!;
  expect(dial.kind).toBe('button');
  expect(compatibleVariants(dial)[0]).toBe('ticks');
  const ev = bodies['x70-110']!.find((input) => input.id === 'exposure_compensation_dial')!;
  expect(compatibleVariants(ev)[0]).toBe('dial');
  const focus = bodies['x70-110']!.find((input) => input.id === 'focus_mode_selector')!;
  expect(compatibleVariants(focus)[0]).toBe('segment');
  const shutter = bodies['x70-110']!.find((input) => input.id === 'shutter_button')!;
  expect(compatibleVariants(shutter)).toEqual(['shutter']);
});
