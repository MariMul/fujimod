import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as THREE from 'three';
import { transformWithOxc } from 'vite';
const input = readFileSync(new URL('./rotary-part.ts', import.meta.url), 'utf8').replace("'three'", JSON.stringify(import.meta.resolve('three')));
const { code: source } = await transformWithOxc(input, 'rotary-part.ts');
const { RotaryPart } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

test('exported body-origin meshes turn about their own center, with visible travel to a detent', () => {
  const root = new THREE.Group(), target = new THREE.Group();
  root.position.set(1, 2, 3); target.position.set(10, 2, 3); root.add(target);
  const geometry = new THREE.CylinderGeometry(1, 1, .2, 20).translate(3, 4, 5);
  const mesh = new THREE.Mesh(geometry); target.add(mesh); root.updateMatrixWorld(true);
  geometry.computeBoundingBox();
  const bounds = geometry.boundingBox.clone(), center = target.localToWorld(bounds.getCenter(new THREE.Vector3()));
  const part = new RotaryPart(target, bounds);
  assert.deepEqual(part.axis.toArray(), [0, 1, 0]);
  assert.deepEqual(part.pivot.position.toArray(), [13, 6, 8]);
  const initial = part.pivot.quaternion.clone(), start = performance.now();
  part.turn(1); part.advance(start + 30);
  assert(part.pivot.quaternion.angleTo(initial) > 0);
  assert(part.pivot.quaternion.angleTo(initial) < Math.PI / 12);
  part.advance(start + 100);
  assert(Math.abs(part.pivot.quaternion.angleTo(initial) - Math.PI / 12) < 1e-7);
  const finalCenter = mesh.localToWorld(bounds.getCenter(new THREE.Vector3()));
  assert(finalCenter.distanceTo(center) < 1e-7);
  assert.equal(part.advance(start + 200), false);
  part.turn(-1); part.advance(start + 300);
  assert(part.pivot.quaternion.angleTo(initial) < 1e-7);
});

test('a held dial follows fractional motion without animation toward a detent', () => {
  const root = new THREE.Group(), target = new THREE.Group(); root.add(target);
  const part = new RotaryPart(target, new THREE.Box3(new THREE.Vector3(-1, -.1, -1), new THREE.Vector3(1, .1, 1)));
  const origin = part.pivot.quaternion.clone();
  for (const fraction of [.25, .5, .75]) {
    part.preview(fraction);
    assert(Math.abs(part.pivot.quaternion.angleTo(origin) - fraction * Math.PI / 12) < 1e-7);
    assert.equal(part.advance(performance.now() + 1000), false);
  }
  part.turn(1); part.preview(.25);
  assert(Math.abs(part.pivot.quaternion.angleTo(origin) - 1.25 * Math.PI / 12) < 1e-7);
  const release = performance.now(); part.settle();
  part.advance(release + 100);
  assert(Math.abs(part.pivot.quaternion.angleTo(origin) - Math.PI / 12) < 1e-7);
});

 test('absolute positions replace prior turns and restore the shared detent immediately', () => {
  const root = new THREE.Group(), target = new THREE.Group(); root.add(target);
  const part = new RotaryPart(target, new THREE.Box3(new THREE.Vector3(-1, -.1, -1), new THREE.Vector3(1, .1, 1)));
  const origin = part.pivot.quaternion.clone();
  part.turn(7);
  part.setPosition(2);
  assert(Math.abs(part.pivot.quaternion.angleTo(origin) - Math.PI / 6) < 1e-7);
  assert.equal(part.advance(performance.now() + 1000), false);
  part.preview(.5);
  part.setPosition(-1);
  assert(Math.abs(part.pivot.quaternion.angleTo(origin) - Math.PI / 12) < 1e-7);
  part.setPosition(0);
  assert(part.pivot.quaternion.angleTo(origin) < 1e-7);
});
