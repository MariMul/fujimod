import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const source = ts.transpileModule(readFileSync(new URL('./rotary-drag.ts', import.meta.url), 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { RotaryDrag } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

class Canvas extends EventTarget {
  captured = new Set();
  setPointerCapture(id) { this.captured.add(id); }
  hasPointerCapture(id) { return this.captured.has(id); }
  releasePointerCapture(id) { this.captured.delete(id); }
}
function pointer(type, x, y, id = 1) {
  const event = new Event(type, { cancelable: true });
  Object.assign(event, { clientX: x, clientY: y, pointerId: id, isPrimary: id === 1, button: 0 });
  return event;
}
function setup() {
  globalThis.window = new EventTarget(); globalThis.document = new EventTarget();
  const canvas = new Canvas(), changes = [], orbit = [];
  const drag = new RotaryDrag(canvas, enabled => orbit.push(enabled));
  return { canvas, changes, orbit, drag, start(x = 0, y = 0) { return drag.begin(pointer('pointerdown', x, y), n => changes.push(n)); } };
}
test('dial consumes every detent, preserves remainder and reverses; a tap has no step', () => {
  const s = setup(); s.start();
  s.canvas.dispatchEvent(pointer('pointermove', 42, 1));
  s.canvas.dispatchEvent(pointer('pointermove', 48, 1));
  s.canvas.dispatchEvent(pointer('pointermove', 16, 1));
  assert.deepEqual(s.changes, [1, 1, 1, -1, -1]);
  s.canvas.dispatchEvent(pointer('pointerup', 16, 1));
  assert.deepEqual(s.orbit, [false, true]); assert.equal(s.drag.dragging, false);
  s.start(); s.canvas.dispatchEvent(pointer('pointerup', 0, 0));
  assert.equal(s.changes.length, 5);
});
test('vertical swipe works and locks direction without diagonal double counting', () => {
  const s = setup(); s.start(50, 100);
  s.canvas.dispatchEvent(pointer('pointermove', 51, 68));
  s.canvas.dispatchEvent(pointer('pointermove', 150, 68));
  assert.deepEqual(s.changes, [1, 1]);
});
test('secondary touch never reaches orbit, cancellation releases the owner', () => {
  const s = setup(); s.start();
  const second = pointer('pointerdown', 100, 100, 2);
  assert.equal(s.drag.begin(second), true); assert.equal(second.defaultPrevented, true);
  s.canvas.dispatchEvent(pointer('pointermove', 30, 0, 2)); assert.deepEqual(s.changes, []);
  s.canvas.dispatchEvent(pointer('pointercancel', 0, 0));
  assert.equal(s.canvas.hasPointerCapture(1), false); assert.equal(s.drag.dragging, false);
  const move = pointer('pointermove', 20, 0, 2); s.canvas.dispatchEvent(move);
  assert.equal(move.defaultPrevented, true);
});
test('empty-space touch is left to orbit; disable and blur stop a drag', () => {
  const s = setup(); const empty = pointer('pointerdown', 0, 0);
  assert.equal(s.drag.begin(empty), false); assert.equal(empty.defaultPrevented, false);
  s.start(); s.drag.setEnabled(false);
  s.canvas.dispatchEvent(pointer('pointermove', 90, 0)); assert.deepEqual(s.changes, []);
  s.drag.setEnabled(true); s.start(); window.dispatchEvent(new Event('blur'));
  assert.equal(s.drag.dragging, false); assert.equal(s.canvas.captured.size, 0);
});
