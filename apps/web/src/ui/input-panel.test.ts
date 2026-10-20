import { expect, test } from 'vite-plus/test';
import { inputPanel } from './input-panel';
test('an unqualified catalog exposes no electronic controls or boot combinations', () => {
  expect(inputPanel([])).toEqual({ combos: [], layout: [] });
});
test('presentation preserves native IDs, stages, detents and refused capabilities', () => {
  const panel = inputPanel([
    { id: 'menu', label: 'MENU', kind: 'button', supported: true },
    {
      id: 'focus',
      label: 'Focus',
      kind: 'switch',
      supported: true,
      positions: [
        { id: 'M', label: 'M' },
        { id: 'S', label: 'S' },
      ],
    },
    {
      id: 'shutter',
      label: 'Shutter',
      kind: 'button',
      supported: true,
      stages: [
        { id: 'half', label: 'Half' },
        { id: 'full', label: 'Full' },
      ],
    },
    { id: 'ring', label: 'Ring', kind: 'rotary', relative: true, supported: true },
    { id: 'unlocated', label: 'Unlocated', kind: 'button', supported: false },
    { id: 'screen', label: 'LCD', kind: 'touch', supported: true },
  ]);
  expect(panel.layout.map((node) => [node.id, node.variant])).toEqual([
    ['menu', 'key'],
    ['focus', 'segment'],
    ['shutter', 'shutter'],
    ['ring', 'dial'],
    ['unlocated', 'key'],
  ]);
  expect(panel.layout[1]!.labels).toEqual({ M: 'M', S: 'S' });
  expect(panel.combos).toEqual([]);
});
