import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { transformWithOxc } from 'vite';
const { code: source } = await transformWithOxc(
  readFileSync(new URL('./rotary-drag.ts', import.meta.url), 'utf8'),
  'rotary-drag.ts',
);
const { RotaryDrag } = await import(
  `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`
);

class Canvas extends EventTarget {
  captured = new Set<number>();
  setPointerCapture(id: number) {
    this.captured.add(id);
  }
  hasPointerCapture(id: number) {
    return this.captured.has(id);
  }
  releasePointerCapture(id: number) {
    this.captured.delete(id);
  }
}
function pointer(type: string, x: number, y: number, id = 1) {
  const event = new Event(type, { cancelable: true });
  Object.assign(event, { clientX: x, clientY: y, pointerId: id, isPrimary: id === 1, button: 0 });
  return event;
}
function setup() {
  Object.assign(globalThis, { window: new EventTarget(), document: new EventTarget() });
  const canvas = new Canvas(),
    changes: number[] = [],
    orbit: boolean[] = [],
    previews: number[] = [],
    finishes: boolean[] = [];
  const drag = new RotaryDrag(canvas, (enabled: boolean) => orbit.push(enabled));
  const gesture: {
    direction?: { x: number; y: number };
    step(n: number): number;
    preview(n: number): number;
    finish(): number;
  } = {
    step: (n: number) => changes.push(n),
    preview: (n: number) => previews.push(n),
    finish: () => finishes.push(true),
  };
  return {
    canvas,
    changes,
    orbit,
    drag,
    previews,
    finishes,
    gesture,
    start(x = 0, y = 0) {
      return drag.begin(pointer('pointerdown', x, y), gesture);
    },
  };
}
test('dial consumes every detent, preserves remainder and reverses; a tap has no step', () => {
  const s = setup();
  s.start();
  s.canvas.dispatchEvent(pointer('pointermove', 42, 1));
  s.canvas.dispatchEvent(pointer('pointermove', 48, 1));
  s.canvas.dispatchEvent(pointer('pointermove', 16, 1));
  assert.deepEqual(s.changes, [1, 1, 1, -1, -1]);
  s.canvas.dispatchEvent(pointer('pointerup', 16, 1));
  assert.deepEqual(s.orbit, [false, true]);
  assert.equal(s.drag.dragging, false);
  s.start();
  s.canvas.dispatchEvent(pointer('pointerup', 0, 0));
  assert.equal(s.changes.length, 5);
});
test('vertical swipe works and locks direction without diagonal double counting', () => {
  const s = setup();
  s.start(50, 100);
  s.canvas.dispatchEvent(pointer('pointermove', 51, 68));
  s.canvas.dispatchEvent(pointer('pointermove', 150, 68));
  assert.deepEqual(s.changes, [1, 1]);
});
test('secondary touch never reaches orbit, cancellation releases the owner', () => {
  const s = setup();
  s.start();
  const second = pointer('pointerdown', 100, 100, 2);
  assert.equal(s.drag.begin(second), true);
  assert.equal(second.defaultPrevented, true);
  s.canvas.dispatchEvent(pointer('pointermove', 30, 0, 2));
  assert.deepEqual(s.changes, []);
  s.canvas.dispatchEvent(pointer('pointercancel', 0, 0));
  assert.equal(s.canvas.hasPointerCapture(1), false);
  assert.equal(s.drag.dragging, false);
  const move = pointer('pointermove', 20, 0, 2);
  s.canvas.dispatchEvent(move);
  assert.equal(move.defaultPrevented, true);
});
test('empty-space touch is left to orbit; disable and blur stop a drag', () => {
  const s = setup();
  const empty = pointer('pointerdown', 0, 0);
  assert.equal(s.drag.begin(empty), false);
  assert.equal(empty.defaultPrevented, false);
  s.start();
  s.drag.setEnabled(false);
  s.canvas.dispatchEvent(pointer('pointermove', 90, 0));
  assert.deepEqual(s.changes, []);
  const disabled = pointer('pointerdown', 0, 0);
  assert.equal(s.drag.begin(disabled, s.gesture), true);
  assert.equal(disabled.defaultPrevented, true);
  s.canvas.dispatchEvent(pointer('pointermove', 90, 0));
  assert.deepEqual(s.changes, []);
  s.drag.setEnabled(false);
  s.drag.setEnabled(true);
  s.start();
  window.dispatchEvent(new Event('blur'));
  assert.equal(s.drag.dragging, false);
  assert.equal(s.canvas.captured.size, 0);
});

test('visual travel follows sub-detent motion and only settles when released', () => {
  const s = setup();
  s.start();
  for (const x of [4, 8, 12]) s.canvas.dispatchEvent(pointer('pointermove', x, 0));
  assert.deepEqual(s.changes, []);
  assert.deepEqual(s.previews, [0, 0.25, 0.5, 0.75]);
  s.canvas.dispatchEvent(pointer('pointermove', 20, 0));
  assert.deepEqual(s.changes, [1]);
  assert.equal(s.previews.at(-1), 0.25);
  assert.deepEqual(s.finishes, []);
  s.canvas.dispatchEvent(pointer('pointerup', 20, 0));
  assert.deepEqual(s.finishes, [true]);
  s.start();
  window.dispatchEvent(new Event('blur'));
  assert.equal(s.finishes.length, 2);
});

test('projected grab tangent follows either side of a dial without axis switching', () => {
  for (const direction of [
    { x: -1, y: 0 },
    { x: 0, y: 1 },
    { x: 0.6, y: 0.8 },
  ]) {
    const s = setup();
    s.gesture.direction = direction;
    s.start(100, 100);
    s.canvas.dispatchEvent(pointer('pointermove', 100 + direction.x * 32, 100 + direction.y * 32));
    assert.deepEqual(s.changes, [1, 1]);
    s.canvas.dispatchEvent(pointer('pointermove', 100, 100));
    assert.deepEqual(s.changes, [1, 1, -1, -1]);
  }
});

test('a swipe completed before picking returns preserves detents and reversal without pointer capture', () => {
  const s = setup();
  s.canvas.setPointerCapture = () => {
    throw Error('Pointer already released');
  };
  s.drag.replay(pointer('pointerdown', 0, 0), s.gesture, [
    { x: 40, y: 0 },
    { x: 48, y: 0 },
    { x: 16, y: 0 },
  ]);
  assert.deepEqual(s.changes, [1, 1, 1, -1, -1]);
  assert.deepEqual(s.finishes, [true]);
  assert.deepEqual(s.orbit, [false, true]);
  assert.equal(s.drag.dragging, false);
});

test('a held pointer consumes movement recorded while its pick was pending', () => {
  const s = setup();
  s.drag.begin(pointer('pointerdown', 0, 0), s.gesture, [{ x: 40, y: 0 }]);
  assert.deepEqual(s.changes, [1, 1]);
  assert.equal(s.previews.at(-1), 0.5);
  s.canvas.dispatchEvent(pointer('pointermove', 48, 0));
  assert.deepEqual(s.changes, [1, 1, 1]);
});
