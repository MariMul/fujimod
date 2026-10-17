import * as THREE from 'three';

/** A model-local pivot; imported GLB nodes often all have the body origin. */
export class RotaryPart {
  readonly pivot = new THREE.Group();
  readonly axis: THREE.Vector3;
  private angle = 0;
  private destination = 0;
  private updatedAt = 0;
  constructor(readonly target: THREE.Object3D, bounds: THREE.Box3, labels: THREE.Object3D[] = []) {
    const extent = bounds.getSize(new THREE.Vector3()).toArray();
    const axisIndex = extent.indexOf(Math.min(...extent));
    this.axis = new THREE.Vector3().setComponent(axisIndex, 1).applyQuaternion(target.quaternion);
    this.pivot.name = `rotary:${target.name}`;
    this.pivot.position.copy(bounds.getCenter(new THREE.Vector3()).applyMatrix4(target.matrix));
    target.parent!.add(this.pivot);
    this.pivot.updateWorldMatrix(true, false);
    for (const node of [target, ...labels]) this.pivot.attach(node);
  }
  turn(direction: number) {
    if (this.angle === this.destination) this.updatedAt = performance.now();
    this.destination += direction * Math.PI / 12;
  }
  /** Travel between detents makes even an unmarked, knurled ring readable. */
  advance(now: number): boolean {
    if (this.angle === this.destination) return false;
    const remaining = this.destination - this.angle;
    const travel = Math.max(0, now - this.updatedAt) * (Math.PI / 12) / 60;
    this.updatedAt = now;
    this.angle = Math.abs(remaining) <= travel ? this.destination : this.angle + Math.sign(remaining) * travel;
    this.pivot.quaternion.setFromAxisAngle(this.axis, this.angle);
    this.pivot.updateWorldMatrix(false, true);
    return true;
  }
}
