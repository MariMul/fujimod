import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
test('Model, panels and recordings share complete control presentations', () => {
  const parts = read('../devices/x70/parts.json');
  const panel = read('../devices/x70/panel.json');
  const { controls, icons } = read('../../../bindings/python/python/fujimod/presentation/controls.json');
  assert.equal(Object.hasOwn(panel, 'inputLabels'), false, 'Names belong only in shared presentation');
  assert.equal(Object.hasOwn(parts, 'labels'), false, 'Names belong only in shared presentation');
  const ids = new Set([...Object.keys(parts.inputs), ...parts.order]);
  for (const regions of Object.values(parts.regions))
    for (const id of Object.values(regions)) ids.add(id);
  const visit = (nodes) => {
    for (const node of nodes) {
      if (node.id) ids.add(node.id);
      for (const id of Object.values(node.bindings ?? {})) ids.add(id);
      visit(node.children ?? []);
    }
  };
  visit(panel.layout);
  visit(read('../devices/xm1/panel.json').layout);
  for (const id of ids) assert.ok(controls[id]?.label.trim(), `Missing name for ${id}`);
  assert.equal(controls.fn7.label, 'Side Fn');
  for (const id of ['up', 'down', 'left', 'right']) {
    assert.equal(controls[id].icon, id);
    assert.equal(Object.hasOwn(controls[id], 'text'), false);
    assert.ok(icons[id].lines.length);
  }
});
