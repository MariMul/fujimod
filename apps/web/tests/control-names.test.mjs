import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
test('X70 model and panel share complete control names', () => {
  const parts = read('../devices/x70/parts.json');
  const panel = read('../devices/x70/panel.json');
  assert.equal(Object.hasOwn(parts, 'labels'), false, 'Names belong only in panel.inputLabels');
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
  for (const id of ids) assert.ok(panel.inputLabels[id]?.trim(), `Missing name for ${id}`);
  assert.equal(panel.inputLabels.fn7, 'Side Fn');
});
