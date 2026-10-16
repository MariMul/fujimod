import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import type { Frame } from '../engine/api';

export interface ModelData {
  url: string;
  yaw: number;
  hidden: string[];
  screen: string;
  inputs: Record<string, string[] | { nodes: string[]; split?: string }>;
  regions: Record<string, Record<string, string>>;
  order: string[];
  labels: Record<string, string>;
  credit: { title: string; author: string; url: string; authorUrl: string; license: string };
}
export interface ModelHit { id: string; point?: { x: number; y: number } }

/** Rendering, picking and LCD texture placement; no firmware knowledge. */
export class ModelViewer {
  readonly renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  private scene = new THREE.Scene();
  private camera = new THREE.PerspectiveCamera(35, 1, 0.05, 100);
  private orbit: OrbitControls;
  private root = new THREE.Group();
  private targets = new Map<string, THREE.Object3D>();
  private lcd = document.createElement('canvas');
  private texture = new THREE.CanvasTexture(this.lcd);
  private ray = new THREE.Raycaster();
  private pointer = new THREE.Vector2();
  private data?: ModelData;
  private generation = 0;
  private home = new THREE.Vector3();
  private homeTarget = new THREE.Vector3();
  private leaderSignature = '';
  private highlightId: string | null = null;
  private dirty = true;
  private leadersDirty = true;
  onPointerDown?: (hit: ModelHit, event: PointerEvent) => void;
  onHover?: (id: string | null) => void;
  constructor(private host: HTMLElement, private lines: SVGSVGElement, private workspace: HTMLElement) {
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    host.prepend(this.renderer.domElement);
    this.camera.position.set(0, 0.6, 6);
    this.orbit = new OrbitControls(this.camera, this.renderer.domElement);
    this.orbit.enableDamping = true;
    this.orbit.addEventListener('change', () => { this.dirty = this.leadersDirty = true; });
    window.addEventListener('scroll', () => { this.leadersDirty = true; }, true);
    this.orbit.minDistance = 2; this.orbit.maxDistance = 10;
    this.scene.add(new THREE.HemisphereLight(0xc9d4e1, 0x28221c, 2.1));
    const key = new THREE.DirectionalLight(0xffffff, 3.4); key.position.set(-3, 5, 5); this.scene.add(key);
    const rim = new THREE.DirectionalLight(0x8899bb, 2.2); rim.position.set(3, 2, -4); this.scene.add(rim);
    this.scene.add(this.root);
    this.lcd.id = 'lcd-source'; this.lcd.hidden = true; host.append(this.lcd);
    this.lcd.width = 720; this.lcd.height = 480;
    this.texture.colorSpace = THREE.SRGBColorSpace;
    this.texture.generateMipmaps = false; this.texture.minFilter = THREE.LinearFilter;
    this.placeholder();
    new ResizeObserver(() => this.resize()).observe(host);
    new MutationObserver(() => { this.leadersDirty = true; this.leaderSignature = ''; }).observe(document.getElementById('controls')!, { childList: true });
    // Capture runs before OrbitControls' bubble handler, preventing control drags from orbiting.
    this.renderer.domElement.addEventListener('pointerdown', event => {
      const hit = this.pick(event);
      if (hit) this.onPointerDown?.(hit, event);
    }, { capture: true });
    this.renderer.domElement.addEventListener('pointermove', event => {
      if (!event.buttons) this.onHover?.(this.pick(event)?.id ?? null);
    });
    this.renderer.domElement.addEventListener('pointerleave', () => this.onHover?.(null));
    this.renderer.setAnimationLoop(() => {
      this.orbit.update();
      if (this.dirty) {
        this.renderer.render(this.scene, this.camera); this.dirty = false;
        this.host.dataset.renderFrames = String(this.renderer.info.render.frame);
      }
      if (this.leadersDirty) { this.projectLeaders(); this.leadersDirty = false; }
    });
  }
  private disposeRoot() {
    this.targets.clear();
    this.root.traverse(object => {
      if (object instanceof THREE.Mesh) {
        object.geometry.dispose();
        for (const material of Array.isArray(object.material) ? object.material : [object.material]) material.dispose();
      }
    });
    this.dirty = this.leadersDirty = true;
    this.root.clear(); this.root.rotation.set(0, 0, 0);
  }
  placeholder() {
    this.generation++; this.data = undefined; this.disposeRoot();
    const material = new THREE.MeshStandardMaterial({ color: 0x050609, roughness: 0.85, metalness: 0.1 });
    const body = new THREE.Mesh(new THREE.BoxGeometry(2.7, 1.55, 0.55), material);
    this.root.add(body);
    const lens = new THREE.Mesh(new THREE.CylinderGeometry(0.57, 0.57, 0.4, 48), material);
    lens.rotation.x = Math.PI / 2; lens.position.z = 0.4; this.root.add(lens);
    const rim = new THREE.Mesh(new THREE.TorusGeometry(.48, .025, 8, 64), new THREE.MeshBasicMaterial({ color: 0x252832 }));
    rim.position.z = .61; this.root.add(rim);
    const top = new THREE.Mesh(new THREE.BoxGeometry(0.8, 0.2, 0.5), material); top.position.y = 0.85; this.root.add(top);
    this.fit();
  }
  async load(data: ModelData) {
    this.data = data;
    const generation = ++this.generation;
    const gltf = await new GLTFLoader().loadAsync(new URL(data.url, document.baseURI).href);
    if (generation !== this.generation) return;
    this.disposeRoot();
    const model = gltf.scene;
    const bounds = new THREE.Box3().setFromObject(model);
    const size = bounds.getSize(new THREE.Vector3());
    const scale = 2.9 / Math.max(size.x, size.y, size.z);
    model.scale.setScalar(scale);
    model.position.copy(bounds.getCenter(new THREE.Vector3())).multiplyScalar(-scale);
    model.traverse(object => {
      if (data.hidden.includes(object.name)) object.visible = false;
      if (object instanceof THREE.Mesh) object.material = Array.isArray(object.material) ? object.material.map(material => material.clone()) : object.material.clone();
    });
    this.root.add(model); this.root.rotation.y = data.yaw;
    const screen = model.getObjectByName(data.screen);
    screen?.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return;
      this.screenUvs(object.geometry);
      object.material = new THREE.MeshBasicMaterial({ map: this.texture, side: THREE.DoubleSide, toneMapped: false });
    });
    for (const [id, mapping] of Object.entries(data.inputs)) {
      for (const name of Array.isArray(mapping) ? mapping : mapping.nodes) {
        const node = model.getObjectByName(name);
        if (!node) continue;
        if (!this.targets.has(id)) this.targets.set(id, node);
        node.userData.inputIds ||= []; node.userData.inputIds.push(id);
      }
    }
    this.root.updateMatrixWorld(true); this.fit();
  }
  private screenUvs(geometry: THREE.BufferGeometry) {
    geometry.computeBoundingBox(); const bounds = geometry.boundingBox!;
    const extent = bounds.getSize(new THREE.Vector3()).toArray();
    const axes = [0, 1, 2].sort((a, b) => extent[b] - extent[a]);
    const uv = new Float32Array(geometry.attributes.position.count * 2);
    const point = new THREE.Vector3();
    for (let i = 0; i < geometry.attributes.position.count; i++) {
      point.fromBufferAttribute(geometry.attributes.position, i);
      uv[i * 2] = 1 - (point.getComponent(axes[0]) - bounds.min.getComponent(axes[0])) / extent[axes[0]];
      uv[i * 2 + 1] = (point.getComponent(axes[1]) - bounds.min.getComponent(axes[1])) / extent[axes[1]];
    }
    geometry.setAttribute('uv', new THREE.BufferAttribute(uv, 2));
  }
  frame(frame: Frame) {
    if (frame.rgb.length !== frame.width * frame.height * 3) throw Error('Invalid LCD frame size.');
    this.lcd.width = frame.width; this.lcd.height = frame.height;
    const context = this.lcd.getContext('2d')!;
    const image = context.createImageData(frame.width, frame.height);
    for (let s = 0, t = 0; s < frame.rgb.length; s += 3, t += 4) {
      image.data[t] = frame.rgb[s]; image.data[t + 1] = frame.rgb[s + 1]; image.data[t + 2] = frame.rgb[s + 2]; image.data[t + 3] = 255;
    }
    context.putImageData(image, 0, 0); this.texture.needsUpdate = true; this.dirty = true;
    this.host.dataset.frames = String(frame.sequence);
  }
  private localBounds(target: THREE.Object3D) {
    target.updateWorldMatrix(true, true);
    const inverse = target.matrixWorld.clone().invert(); const box = new THREE.Box3();
    target.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return;
      object.geometry.computeBoundingBox(); const b = object.geometry.boundingBox!;
      const transform = inverse.clone().multiply(object.matrixWorld);
      for (let c = 0; c < 8; c++) box.expandByPoint(new THREE.Vector3(c & 1 ? b.max.x : b.min.x, c & 2 ? b.max.y : b.min.y, c & 4 ? b.max.z : b.min.z).applyMatrix4(transform));
    });
    return box;
  }
  private pick(event: PointerEvent): ModelHit | null {
    if (!this.data) return null;
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.set((event.clientX - rect.left) / rect.width * 2 - 1, -(event.clientY - rect.top) / rect.height * 2 + 1);
    this.ray.setFromCamera(this.pointer, this.camera);
    const hit = this.ray.intersectObjects(this.root.children, true).find(hit => this.visible(hit.object));
    if (!hit) return null;
    let node: THREE.Object3D | null = hit.object;
    while (node && !node.userData.inputIds) node = node.parent;
    if (!node) return null;
    const bounds = this.localBounds(node); const local = node.worldToLocal(hit.point.clone());
    const x = (local.x - bounds.min.x) / Math.max(1e-6, bounds.max.x - bounds.min.x);
    const y = (local.y - bounds.min.y) / Math.max(1e-6, bounds.max.y - bounds.min.y);
    const region = this.data.regions[node.name];
    if (region) {
      const dx = x - 0.5, dy = y - 0.5;
      const direction = Math.max(Math.abs(dx), Math.abs(dy)) <= 0.22 ? 'center' : Math.abs(dx) > Math.abs(dy) ? dx < 0 ? 'left' : 'right' : dy < 0 ? 'down' : 'up';
      return { id: region[direction] };
    }
    return { id: node.userData.inputIds[0], point: hit.uv ? { x: hit.uv.x, y: 1 - hit.uv.y } : undefined };
  }
  private visible(node: THREE.Object3D) { for (let n: THREE.Object3D | null = node; n; n = n.parent) if (!n.visible) return false; return true; }
  highlight(id: string | null) {
    if (this.highlightId === id) return;
    this.dirty = this.leadersDirty = true;
    this.highlightId = id;
    this.root.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return;
      const materials = Array.isArray(object.material) ? object.material : [object.material];
      for (const material of materials) if (material instanceof THREE.MeshStandardMaterial) {
        if (!material.userData.baseEmissive) material.userData.baseEmissive = material.emissive.clone();
        const target = id && this.targets.get(id);
        let selected = false;
        for (let n: THREE.Object3D | null = object; n; n = n.parent) if (n === target) selected = true;
        material.emissive.copy(selected ? new THREE.Color(0x0A84FF) : material.userData.baseEmissive);
      }
    });
  }
  setOrbit(enabled: boolean) { this.orbit.enabled = enabled; }
  faceScreen() { this.camera.position.copy(this.home); this.orbit.target.copy(this.homeTarget); this.orbit.update(); }
  faceLens() {
    this.camera.position.copy(this.home); this.camera.position.z = this.homeTarget.z - (this.home.z - this.homeTarget.z);
    this.orbit.target.copy(this.homeTarget); this.orbit.update();
  }
  private fit() {
    this.dirty = this.leadersDirty = true;
    this.root.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(this.root), size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3()), tangent = Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2));
    const distance = (Math.max(size.y / (2 * tangent), size.x / (2 * tangent * this.camera.aspect)) + size.z / 2) * 1.12;
    this.orbit.target.copy(center); this.camera.position.copy(center).add(new THREE.Vector3(0, 0, distance));
    this.home.copy(this.camera.position); this.homeTarget.copy(center); this.orbit.update();
  }
  private resize() {
    const rect = this.host.getBoundingClientRect();
    this.renderer.setSize(rect.width, rect.height, false); this.camera.aspect = rect.width / rect.height; this.camera.updateProjectionMatrix(); this.fit();
  }
  private projectLeaders() {
    const signature = [this.camera.position.toArray(), this.camera.quaternion.toArray(), this.host.clientWidth, this.host.clientHeight, document.querySelector('#panel')?.scrollTop, window.scrollY, this.highlightId, this.generation].join('|');
    if (signature === this.leaderSignature) return;
    this.leaderSignature = signature;
    this.lines.replaceChildren();
    if (matchMedia('(max-width: 760px)').matches || !this.data) return;
    const wr = this.workspace.getBoundingClientRect(), vr = this.host.getBoundingClientRect();
    this.lines.setAttribute('viewBox', `0 0 ${wr.width} ${wr.height}`);
    const origin = this.camera.getWorldPosition(new THREE.Vector3());
    const list = document.querySelector('#controls')!.getBoundingClientRect();
    for (const [id, target] of this.targets) {
      const row = document.querySelector<HTMLElement>(`.control-row[data-id="${CSS.escape(id)}"]`);
      if (!row) continue;
      const rr = row.getBoundingClientRect();
      if (rr.top < list.top || rr.bottom > list.bottom || rr.bottom > innerHeight) continue;
      let p = new THREE.Box3().setFromObject(target).getCenter(new THREE.Vector3());
      const regions = this.data.regions[target.name];
      if (regions) {
        const region = Object.entries(regions).find(([, input]) => input === id)?.[0] ?? 'center';
        const offsets: Record<string, [number, number]> = { up: [.5, .81], down: [.5, .19], left: [.19, .5], right: [.81, .5], center: [.5, .5] };
        const [x, y] = offsets[region] ?? offsets.center;
        const box = this.localBounds(target), size = box.getSize(new THREE.Vector3());
        p = target.localToWorld(new THREE.Vector3(box.min.x + size.x * x, box.min.y + size.y * y, box.min.z + size.z * .08));
      }
      const delta = p.clone().sub(origin); this.ray.set(origin, delta.clone().normalize());
      this.ray.far = delta.length() + 0.02;
      const hit = this.ray.intersectObjects(this.root.children, true).find(hit => this.visible(hit.object));
      this.ray.far = Infinity;
      let belongs = false;
      for (let n: THREE.Object3D | null = hit?.object ?? null; n; n = n.parent) if (n === target) belongs = true;
      if (!belongs) continue;
      p.project(this.camera);
      const x = vr.left - wr.left + (p.x * 0.5 + 0.5) * vr.width, y = vr.top - wr.top + (-p.y * 0.5 + 0.5) * vr.height;
      const endX = rr.left - wr.left, endY = rr.top + rr.height / 2 - wr.top;
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.dataset.input = id; path.dataset.x = String(x); path.dataset.y = String(y);
      path.setAttribute('d', `M${x},${y} L${endX - 18},${endY} L${endX},${endY}`);
      if (row.classList.contains('active')) path.classList.add('active');
      this.lines.append(path);
    }
  }
}
