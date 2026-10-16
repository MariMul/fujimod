import * as THREE from 'three';

/** A model-local pivot; imported GLB nodes often all have the body origin. */
export class RotaryPart {
  readonly pivot = new THREE.Group();
  readonly axis: THREE.Vector3;
  private angle = 0;
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
    this.angle += direction * Math.PI / 12;
    this.pivot.quaternion.setFromAxisAngle(this.axis, this.angle);
    this.pivot.updateWorldMatrix(false, true);
  }
}
