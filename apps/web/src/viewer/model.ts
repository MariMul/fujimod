import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Frame, Input } from "../engine/api";
import { DetentFeedback } from "../feedback/detent";
import { RotaryDrag } from "./rotary-drag";
import { RotaryPart } from "./rotary-part";

export interface ModelData {
  url: string;
  yaw: number;
  hidden: string[];
  screen: string;
  inputs: Record<string, string[] | { nodes: string[]; split?: string }>;
  regions: Record<string, Record<string, string>>;
  order: string[];
  labels: Record<string, string>;
  credit: {
    title: string;
    author: string;
    url: string;
    authorUrl: string;
    license: string;
  };
}
export interface ModelHit {
  id: string;
  point?: { x: number; y: number };
  surfacePoint?: THREE.Vector3;
}

/** Rendering, picking and LCD texture placement; no firmware knowledge. */
export class ModelViewer {
  private phoneViewport = matchMedia("(max-width: 760px)").matches;
  readonly renderer = new THREE.WebGLRenderer({
    antialias: !this.phoneViewport,
    alpha: true,
  });
  private scene = new THREE.Scene();
  private camera = new THREE.PerspectiveCamera(35, 1, 0.05, 100);
  private orbit: OrbitControls;
  private root = new THREE.Group();
  private targets = new Map<string, THREE.Object3D>();
  private lcd = document.createElement("canvas");
  private lcdImage?: ImageData;
  private texture = new THREE.CanvasTexture(this.lcd);
  private ray = new THREE.Raycaster();
  private pointer = new THREE.Vector2();
  private data?: ModelData;
  private generation = 0;
  private home = new THREE.Vector3();
  private homeTarget = new THREE.Vector3();
  private leaderSignature = "";
  private highlightId: string | null = null;
  private buttonInputs = new Set<string>();
  private inputMarker = document.createElement("div");
  private inputFeedback = document.createElement("div");
  private feedbackId?: string;
  private feedbackUntil = 0;
  private feedbackPoint?: THREE.Vector3;
  private faulted = false;
  private undoFault?: () => void;
  private orientation?: {
    start: number;
    from: THREE.Vector3;
    to: THREE.Vector3;
    target: THREE.Vector3;
  };
  private rotary = new Map<
    string,
    { part: RotaryPart; negative: string; positive: string }
  >();
  private rotaryParts = new Set<RotaryPart>();
  private rotaryStep?: (id: string, direction: number) => boolean;
  private rotaryDrag: RotaryDrag;
  private detent = new DetentFeedback();
  private dirty = true;
  private leadersDirty = true;
  onPointerDown?: (hit: ModelHit, event: PointerEvent) => void;
  onHover?: (id: string | null) => void;
  constructor(
    private host: HTMLElement,
    private lines: SVGSVGElement,
    private workspace: HTMLElement,
  ) {
    this.renderer.setPixelRatio(
      Math.min(devicePixelRatio, this.phoneViewport ? 1.5 : 2),
    );
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    host.prepend(this.renderer.domElement);
    this.inputMarker.className = "model-input-marker";
    this.inputFeedback.className = "model-input-feedback";
    this.inputFeedback.setAttribute("role", "status");
    this.inputFeedback.setAttribute("aria-live", "polite");
    this.inputMarker.hidden = this.inputFeedback.hidden = true;
    host.append(this.inputMarker, this.inputFeedback);
    this.camera.position.set(0, 0.6, 6);
    this.orbit = new OrbitControls(this.camera, this.renderer.domElement);
    // No residual orbit velocity may move the camera during a dial gesture.
    this.orbit.enableDamping = false;
    this.rotaryDrag = new RotaryDrag(this.renderer.domElement, (enabled) =>
      this.setOrbit(enabled),
    );
    this.orbit.addEventListener("change", () => {
      this.dirty = this.leadersDirty = true;
    });
    window.addEventListener(
      "scroll",
      () => {
        this.leadersDirty = true;
      },
      true,
    );
    this.orbit.minDistance = 2;
    this.orbit.maxDistance = 10;
    this.scene.add(new THREE.HemisphereLight(0xc9d4e1, 0x28221c, 2.1));
    const key = new THREE.DirectionalLight(0xffffff, 3.4);
    key.position.set(-3, 5, 5);
    this.scene.add(key);
    const rim = new THREE.DirectionalLight(0x8899bb, 2.2);
    rim.position.set(3, 2, -4);
    this.scene.add(rim);
    this.scene.add(this.root);
    this.lcd.id = "lcd-source";
    this.lcd.hidden = true;
    host.append(this.lcd);
    this.lcd.width = 720;
    this.lcd.height = 480;
    this.renderer.domElement.classList.add("model-canvas");
    this.lcd.addEventListener("pointerdown", (event) => {
      if (!this.faulted) return;
      const r = this.lcd.getBoundingClientRect();
      const scale = Math.min(
        r.width / this.lcd.width,
        r.height / this.lcd.height,
      );
      const width = this.lcd.width * scale,
        height = this.lcd.height * scale;
      this.faultClick(
        (event.clientX - r.left - (r.width - width) / 2) / width,
        (event.clientY - r.top - (r.height - height) / 2) / height,
      );
    });
    this.texture.colorSpace = THREE.SRGBColorSpace;
    this.texture.generateMipmaps = false;
    this.texture.minFilter = THREE.LinearFilter;
    this.placeholder();
    new ResizeObserver(() => this.resize()).observe(host);
    new MutationObserver(() => {
      this.leadersDirty = true;
      this.leaderSignature = "";
    }).observe(document.getElementById("controls")!, { childList: true });
    // Capture runs before OrbitControls' bubble handler, preventing control drags from orbiting.
    this.renderer.domElement.addEventListener(
      "pointerdown",
      (event) => {
        const hit = this.pick(event);
        if (this.faulted) {
          if (hit?.point) this.faultClick(hit.point.x, hit.point.y);
          return;
        }
        const rotary = hit && this.rotary.get(hit.id);
        const turn = rotary
          ? (direction: number) => {
              const id = direction < 0 ? rotary.negative : rotary.positive;
              if (
                !this.rotaryStep?.(
                  id,
                  rotary.negative === rotary.positive ? direction : 1,
                )
              )
                return;
              rotary.part.turn(direction);
              this.detent.step();
              this.dirty = this.leadersDirty = true;
              this.highlight(id);
              this.onHover?.(id);
              this.leaderSignature = "";
            }
          : undefined;
        const gesture = rotary && turn ? {
          step: turn,
          preview: (fraction: number) => {
            rotary.part.preview(fraction);
            this.dirty = this.leadersDirty = true;
          },
          finish: () => {
            rotary.part.settle();
            this.dirty = this.leadersDirty = true;
          },
        } : undefined;
        if (this.rotaryDrag.begin(event, gesture)) {
          if (rotary) {
            this.detent.beginGesture(event);
            this.onHover?.(hit!.id);
          }
          return;
        }
        if (hit) this.onPointerDown?.(hit, event);
      },
      { capture: true },
    );
    this.renderer.domElement.addEventListener("pointermove", (event) => {
      if (event.pointerType !== "touch" && !event.buttons && !this.rotaryDrag.dragging)
        this.onHover?.(this.pick(event)?.id ?? null);
    });
    this.renderer.domElement.addEventListener("pointerleave", () =>
      this.onHover?.(null),
    );
    this.renderer.setAnimationLoop((now) => {
      if (this.feedbackId && now >= this.feedbackUntil) {
        this.feedbackId = undefined;
        this.inputFeedback.hidden = true;
        this.leadersDirty = true;
      }
      if (this.orientation) {
        const a = this.orientation,
          t = Math.min(1, (now - a.start) / 350),
          ease = t * t * (3 - 2 * t);
        this.camera.position.lerpVectors(a.from, a.to, ease);
        this.orbit.target.copy(a.target);
        this.dirty = this.leadersDirty = true;
        if (t === 1) this.orientation = undefined;
      }
      this.orbit.update();
      for (const part of this.rotaryParts) {
        if (part.advance(now)) this.dirty = this.leadersDirty = true;
      }
      if (this.dirty) {
        this.renderer.render(this.scene, this.camera);
        this.dirty = false;
        this.host.dataset.renderFrames = String(
          this.renderer.info.render.frame,
        );
      }
      if (this.leadersDirty) {
        this.projectInputMarker();
        this.projectLeaders();
        this.leadersDirty = false;
      }
    });
  }
  private disposeRoot() {
    this.rotaryDrag.cancel();
    this.rotary.clear();
    this.rotaryParts.clear();
    this.rotaryStep = undefined;
    this.targets.clear();
    this.buttonInputs.clear();
    this.highlightId = null;
    this.feedbackId = undefined;
    this.inputMarker.hidden = this.inputFeedback.hidden = true;
    this.root.traverse((object) => {
      if (object instanceof THREE.Mesh) {
        object.geometry.dispose();
        for (const material of Array.isArray(object.material)
          ? object.material
          : [object.material])
          material.dispose();
      }
    });
    this.dirty = this.leadersDirty = true;
    this.root.clear();
    this.root.rotation.set(0, 0, 0);
  }
  placeholder() {
    this.generation++;
    this.data = undefined;
    this.disposeRoot();
    const material = new THREE.MeshStandardMaterial({
      color: 0x050609,
      roughness: 0.85,
      metalness: 0.1,
    });
    const body = new THREE.Mesh(
      new THREE.BoxGeometry(2.7, 1.55, 0.55),
      material,
    );
    this.root.add(body);
    const lens = new THREE.Mesh(
      new THREE.CylinderGeometry(0.57, 0.57, 0.4, 48),
      material,
    );
    lens.rotation.x = Math.PI / 2;
    lens.position.z = 0.4;
    this.root.add(lens);
    const rim = new THREE.Mesh(
      new THREE.TorusGeometry(0.48, 0.025, 8, 64),
      new THREE.MeshBasicMaterial({ color: 0x252832 }),
    );
    rim.position.z = 0.61;
    this.root.add(rim);
    const top = new THREE.Mesh(new THREE.BoxGeometry(0.8, 0.2, 0.5), material);
    top.position.y = 0.85;
    this.root.add(top);
    this.fit();
  }
  async load(data: ModelData) {
    this.data = data;
    const generation = ++this.generation;
    const gltf = await new GLTFLoader().loadAsync(
      new URL(data.url, document.baseURI).href,
    );
    if (generation !== this.generation) return;
    this.disposeRoot();
    const model = gltf.scene;
    const bounds = new THREE.Box3().setFromObject(model);
    const size = bounds.getSize(new THREE.Vector3());
    const scale = 2.9 / Math.max(size.x, size.y, size.z);
    model.scale.setScalar(scale);
    model.position
      .copy(bounds.getCenter(new THREE.Vector3()))
      .multiplyScalar(-scale);
    model.traverse((object) => {
      if (data.hidden.includes(object.name)) object.visible = false;
      if (object instanceof THREE.Mesh)
        object.material = Array.isArray(object.material)
          ? object.material.map((material) => material.clone())
          : object.material.clone();
    });
    this.root.add(model);
    this.root.rotation.y = data.yaw;
    const screen = model.getObjectByName(data.screen);
    screen?.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      this.screenUvs(object.geometry);
      object.material = new THREE.MeshBasicMaterial({
        map: this.texture,
        side: THREE.DoubleSide,
        toneMapped: false,
      });
    });
    for (const [id, mapping] of Object.entries(data.inputs)) {
      for (const name of Array.isArray(mapping) ? mapping : mapping.nodes) {
        const node = model.getObjectByName(name);
        if (!node) continue;
        if (!this.targets.has(id)) this.targets.set(id, node);
        node.userData.inputIds ||= [];
        node.userData.inputIds.push(id);
      }
    }
    this.root.updateMatrixWorld(true);
    this.fit();
  }
  /** The panel supplies input policy; all detents use its engine API path. */
  configureRotary(
    inputs: Input[],
    onStep: (id: string, direction: number) => boolean,
  ) {
    this.rotaryDrag.cancel();
    this.rotaryStep = onStep;
    const supported = new Map(
      inputs
        .filter((input) => input.supported)
        .map((input) => [input.id, input]),
    );
    this.buttonInputs = new Set([...supported.values()]
      .filter(input => input.kind.includes("button")).map(input => input.id));
    for (const input of supported.values()) {
      if (input.kind !== "dial" && input.kind !== "ring") continue;
      this.addRotary(input.id, input.id, input.id);
    }
    // A two-direction control with a center press shares one mesh. The part
    // graph distinguishes this from a four-way selector without camera IDs.
    for (const [name, regions] of Object.entries(this.data?.regions ?? {})) {
      if (
        regions.up !== regions.center ||
        regions.down !== regions.center ||
        regions.left === regions.right
      )
        continue;
      if (!supported.has(regions.left) || !supported.has(regions.right))
        continue;
      if (this.targets.get(regions.left)?.name !== name) continue;
      this.addRotary(regions.center, regions.left, regions.right);
    }
    this.setRotaryEnabled(true);
  }
  setRotaryEnabled(enabled: boolean) {
    this.rotaryDrag.setEnabled(enabled);
  }
  private addRotary(id: string, negative: string, positive: string) {
    if (this.rotary.has(id)) return;
    const target = this.targets.get(id) ?? this.targets.get(negative);
    if (!target?.parent) return;
    const bounds = this.localBounds(target),
      size = bounds.getSize(new THREE.Vector3());
    const extents = size.toArray();
    // Transparent textured decals may be exported as sibling nodes. Move
    // contained decals with their part, leaving solid guides and switches fixed.
    const labels = target.parent.children.filter((node) => {
      if (node === target || !this.visible(node) || node.userData.inputIds)
        return false;
      const box = this.localBounds(node)
        .applyMatrix4(node.matrix)
        .applyMatrix4(target.matrix.clone().invert());
      let decal = false,
        solid = false;
      node.traverse((object) => {
        if (!(object instanceof THREE.Mesh)) return;
        for (const material of Array.isArray(object.material)
          ? object.material
          : [object.material]) {
          if (material.transparent && "map" in material && material.map)
            decal = true;
          else solid = true;
        }
      });
      if (box.isEmpty() || !decal || solid) return false;
      const expanded = bounds
        .clone()
        .expandByScalar(Math.max(...extents) * 0.025);
      return expanded.containsBox(box);
    });
    for (const label of labels) label.userData.inputIds = [id];
    const rotary = {
      part: new RotaryPart(target, bounds, labels),
      negative,
      positive,
    };
    this.rotaryParts.add(rotary.part);
    for (const alias of [id, negative, positive])
      this.rotary.set(alias, rotary);
  }
  private screenUvs(geometry: THREE.BufferGeometry) {
    geometry.computeBoundingBox();
    const bounds = geometry.boundingBox!;
    const extent = bounds.getSize(new THREE.Vector3()).toArray();
    const axes = [0, 1, 2].sort((a, b) => extent[b] - extent[a]);
    const uv = new Float32Array(geometry.attributes.position.count * 2);
    const point = new THREE.Vector3();
    for (let i = 0; i < geometry.attributes.position.count; i++) {
      point.fromBufferAttribute(geometry.attributes.position, i);
      uv[i * 2] =
        1 -
        (point.getComponent(axes[0]) - bounds.min.getComponent(axes[0])) /
          extent[axes[0]];
      uv[i * 2 + 1] =
        (point.getComponent(axes[1]) - bounds.min.getComponent(axes[1])) /
        extent[axes[1]];
    }
    geometry.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  }
  frame(frame: Frame) {
    if (frame.rgb.length !== frame.width * frame.height * 3)
      throw Error("Invalid LCD frame size.");
    if (this.lcd.width !== frame.width || this.lcd.height !== frame.height) {
      this.lcd.width = frame.width;
      this.lcd.height = frame.height;
      this.lcdImage = undefined;
    }
    const context = this.lcd.getContext("2d")!;
    const image = (this.lcdImage ??= context.createImageData(
      frame.width,
      frame.height,
    ));
    for (let s = 0, t = 0; s < frame.rgb.length; s += 3, t += 4) {
      image.data[t] = frame.rgb[s];
      image.data[t + 1] = frame.rgb[s + 1];
      image.data[t + 2] = frame.rgb[s + 2];
      image.data[t + 3] = 255;
    }
    context.putImageData(image, 0, 0);
    this.texture.needsUpdate = true;
    this.dirty = true;
    if (this.faulted) this.paintFault();
    this.host.dataset.frames = String(frame.sequence);
  }
  private localBounds(target: THREE.Object3D) {
    target.updateWorldMatrix(true, true);
    const inverse = target.matrixWorld.clone().invert();
    const box = new THREE.Box3();
    target.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      object.geometry.computeBoundingBox();
      const b = object.geometry.boundingBox!;
      const transform = inverse.clone().multiply(object.matrixWorld);
      for (let c = 0; c < 8; c++)
        box.expandByPoint(
          new THREE.Vector3(
            c & 1 ? b.max.x : b.min.x,
            c & 2 ? b.max.y : b.min.y,
            c & 4 ? b.max.z : b.min.z,
          ).applyMatrix4(transform),
        );
    });
    return box;
  }
  private pick(event: PointerEvent): ModelHit | null {
    if (!this.data) return null;
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.set(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      (-(event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.ray.setFromCamera(this.pointer, this.camera);
    const hit = this.ray
      .intersectObjects(this.root.children, true)
      .find((hit) => this.visible(hit.object));
    let node: THREE.Object3D | null = hit?.object ?? null;
    while (node && !node.userData.inputIds) node = node.parent;
    // Decals and the body can cover a button's edge. On touch, also consider
    // nearby visible button centers; never reach through the camera body.
    if (!this.faulted && (!node || node.name !== this.data.screen
      && !node.userData.inputIds.some((id: string) => this.buttonInputs.has(id)))) {
      const nearby = this.nearbyButton(event);
      if (nearby) return nearby;
    }
    if (!node || !hit) return null;
    const bounds = this.localBounds(node);
    const local = node.worldToLocal(hit.point.clone());
    const x =
      (local.x - bounds.min.x) / Math.max(1e-6, bounds.max.x - bounds.min.x);
    const y =
      (local.y - bounds.min.y) / Math.max(1e-6, bounds.max.y - bounds.min.y);
    const region = this.data.regions[node.name];
    if (region) {
      const dx = x - 0.5,
        dy = y - 0.5;
      const direction =
        Math.max(Math.abs(dx), Math.abs(dy)) <= 0.22
          ? "center"
          : Math.abs(dx) > Math.abs(dy)
            ? dx < 0
              ? "left"
              : "right"
            : dy < 0
              ? "down"
              : "up";
      return { id: region[direction] };
    }
    return {
      id: node.userData.inputIds[0],
      point: hit.uv ? { x: hit.uv.x, y: 1 - hit.uv.y } : undefined,
      surfacePoint: this.root.worldToLocal(hit.point.clone()),
    };
  }
  private regionPoint(target: THREE.Object3D, id?: string) {
    const bounds = this.localBounds(target), size = bounds.getSize(new THREE.Vector3());
    const region = Object.entries(this.data?.regions[target.name] ?? {}).find(([, value]) => value === id)?.[0];
    const offsets: Record<string, [number, number]> = {
      up: [.5, .81], down: [.5, .19], left: [.19, .5], right: [.81, .5], center: [.5, .5],
    };
    const [x, y] = offsets[region ?? "center"] ?? offsets.center;
    return target.localToWorld(new THREE.Vector3(bounds.min.x + size.x * x,
      bounds.min.y + size.y * y, bounds.min.z + size.z * .08));
  }
  private projectPoint(point: THREE.Vector3) {
    const rect = this.renderer.domElement.getBoundingClientRect(), p = point.clone().project(this.camera);
    return { x: rect.left + (p.x * .5 + .5) * rect.width,
      y: rect.top + (-p.y * .5 + .5) * rect.height, z: p.z };
  }
  private targetVisible(target: THREE.Object3D, point: THREE.Vector3) {
    const origin = this.camera.getWorldPosition(new THREE.Vector3());
    this.ray.set(origin, point.clone().sub(origin).normalize());
    const hits = this.ray.intersectObjects(this.root.children, true)
      .filter(hit => this.visible(hit.object));
    const own = hits.find(hit => {
      for (let node: THREE.Object3D | null = hit.object; node; node = node.parent)
        if (node === target) return true;
      return false;
    });
    // Permit a thin label on the same button, but not an intervening body panel.
    return Boolean(own && own.distance <= hits[0].distance + .012);
  }
  private nearbyButton(event: PointerEvent): ModelHit | null {
    let best: { target: THREE.Object3D; id: string; distance: number } | undefined;
    const visited = new Set<THREE.Object3D>();
    for (const id of this.buttonInputs) {
      const target = this.targets.get(id);
      if (!target || visited.has(target) || !this.visible(target)) continue;
      visited.add(target);
      const point = this.regionPoint(target), center = this.projectPoint(point);
      if (center.z < -1 || center.z > 1) continue;
      const distance = Math.hypot(event.clientX - center.x, event.clientY - center.y);
      const radius = event.pointerType === "touch"
        ? this.data?.regions[target.name] ? 32 : 22
        : 8;
      if (distance > radius || best && distance >= best.distance) continue;
      if (!this.targetVisible(target, point)) continue;
      best = { target, id, distance };
    }
    if (!best) return null;
    const regions = this.data?.regions[best.target.name];
    if (!regions) return { id: best.id };
    // Use the projected axes so regions follow the camera while it is orbited.
    const center = this.projectPoint(this.regionPoint(best.target));
    const right = this.projectPoint(this.regionPoint(best.target, regions.right));
    const up = this.projectPoint(this.regionPoint(best.target, regions.up));
    const rx = right.x - center.x, ry = right.y - center.y,
      ux = up.x - center.x, uy = up.y - center.y,
      dx = event.clientX - center.x, dy = event.clientY - center.y,
      determinant = rx * uy - ry * ux;
    if (Math.abs(determinant) < .01) return { id: regions.center };
    const x = (dx * uy - dy * ux) / determinant, y = (rx * dy - ry * dx) / determinant;
    const direction = Math.max(Math.abs(x), Math.abs(y)) <= .7 ? "center"
      : Math.abs(x) > Math.abs(y) ? x < 0 ? "left" : "right" : y < 0 ? "down" : "up";
    return { id: regions[direction] };
  }
  /** Immediate acknowledgment, retained after release so a finger cannot hide it. */
  pressFeedback(id: string, surfacePoint?: THREE.Vector3) {
    this.feedbackPoint = surfacePoint?.clone();
    this.feedbackId = id;
    this.feedbackUntil = performance.now() + 1100;
    this.inputFeedback.textContent = this.inputLabel(id);
    this.inputFeedback.dataset.input = id;
    this.inputFeedback.hidden = false;
    this.leadersDirty = true;
  }
  private inputLabel(id: string) {
    return this.data?.labels[id] ?? id;
  }
  private projectInputMarker() {
    const id = this.feedbackId ?? this.highlightId?.split("|")[0], target = id && this.targets.get(id);
    this.inputMarker.hidden = !target || this.host.classList.contains("lcd-mode");
    if (!target || !id) return;
    const touch = Boolean(this.feedbackId && this.feedbackPoint);
    if (target.name === this.data?.screen && !touch) {
      this.inputMarker.hidden = true;
      return;
    }
    const world = touch
      ? this.root.localToWorld(this.feedbackPoint!.clone())
      : this.regionPoint(target, id);
    const point = this.projectPoint(world);
    if (point.z < -1 || point.z > 1 || !this.targetVisible(target, world)) {
      this.inputMarker.hidden = true;
      return;
    }
    const rect = this.host.getBoundingClientRect();
    this.inputMarker.style.left = `${point.x - rect.left}px`;
    this.inputMarker.style.top = `${point.y - rect.top}px`;
    this.inputMarker.dataset.input = id;
    this.inputMarker.textContent = touch ? "" : this.inputLabel(id).match(/^[↑↓←→]/)?.[0] ?? "";
    this.inputMarker.classList.toggle("touch-crosshair", touch);
    this.inputMarker.classList.toggle("pressed", Boolean(this.feedbackId));
  }
  private visible(node: THREE.Object3D) {
    for (let n: THREE.Object3D | null = node; n; n = n.parent)
      if (!n.visible) return false;
    return true;
  }
  highlight(value: string | string[] | null) {
    const ids = typeof value === "string" ? [value] : (value ?? []);
    const id = ids.join("|") || null;
    if (this.highlightId === id) return;
    this.dirty = this.leadersDirty = true;
    this.highlightId = id;
    this.root.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      const materials = Array.isArray(object.material)
        ? object.material
        : [object.material];
      for (const material of materials)
        if (material instanceof THREE.MeshStandardMaterial) {
          if (!material.userData.baseEmissive)
            material.userData.baseEmissive = material.emissive.clone();
          const targets = ids.map((id) => this.targets.get(id))
            .filter(target => target && !this.data?.regions[target.name]);
          let selected = false;
          for (let n: THREE.Object3D | null = object; n; n = n.parent)
            if (targets.includes(n)) selected = true;
          material.emissive.copy(
            selected
              ? new THREE.Color(0x777777)
              : material.userData.baseEmissive,
          );
        }
    });
  }
  setOrbit(enabled: boolean) {
    this.orbit.enabled = enabled && !this.rotaryDrag?.dragging;
  }
  faceScreen() {
    this.orient(this.home.clone());
  }
  faceLens() {
    const position = this.home.clone();
    position.z = this.homeTarget.z - (this.home.z - this.homeTarget.z);
    this.orient(position);
  }
  private orient(to: THREE.Vector3) {
    this.orientation = {
      start: performance.now(),
      from: this.camera.position.clone(),
      to,
      target: this.homeTarget.clone(),
    };
  }
  view(mode: "3d" | "lcd") {
    this.host.classList.toggle("lcd-mode", mode === "lcd");
    this.lcd.hidden = mode !== "lcd";
    this.lcd.classList.toggle("flat", mode === "lcd");
    this.dirty = true;
  }
  fault(active: boolean, undo?: () => void) {
    this.faulted = active;
    this.undoFault = undo;
    if (this.lcdImage)
      this.lcd.getContext("2d")!.putImageData(this.lcdImage, 0, 0);
    if (active) this.paintFault();
    this.texture.needsUpdate = true;
    this.dirty = true;
  }
  private faultClick(x: number, y: number) {
    if (x > 0.5 && x < 0.7 && y > 0.53 && y < 0.65) this.undoFault?.();
  }
  private paintFault() {
    const c = this.lcd.getContext("2d")!,
      w = this.lcd.width,
      h = this.lcd.height,
      scale = w / 720,
      x = (w - 270 * scale) / 2,
      y = (h - 108 * scale) / 2;
    c.save();
    c.translate(x, y);
    c.scale(scale, scale);
    c.fillStyle = "#000";
    c.fillRect(-4, -4, 278, 116);
    c.fillStyle = "#fff";
    c.fillRect(-3, -3, 276, 114);
    c.strokeStyle = "#000";
    c.lineWidth = 1;
    c.strokeRect(0.5, 0.5, 269, 107);
    c.fillStyle = "#000";
    c.beginPath();
    c.arc(32, 43, 17, 0, Math.PI * 2);
    c.fill();
    c.save();
    c.translate(45, 25);
    c.rotate(Math.PI / 4);
    c.fillRect(-5, -5, 10, 10);
    c.restore();
    c.beginPath();
    c.moveTo(49, 23);
    c.quadraticCurveTo(53, 12, 62, 16);
    c.lineWidth = 2;
    c.stroke();
    c.fillStyle = "#fff";
    c.beginPath();
    c.arc(25, 36, 4, 0, Math.PI * 2);
    c.fill();
    c.fillStyle = "#000";
    c.font = "600 16px system-ui";
    c.fillText("Sorry, the emulator", 65, 31);
    c.fillText("stopped.", 65, 51);
    if (this.undoFault) {
      c.lineWidth = 2;
      c.beginPath();
      c.roundRect(180, 72, 70, 24, 7);
      c.stroke();
      c.beginPath();
      c.roundRect(176, 68, 78, 32, 10);
      c.stroke();
      c.font = "600 15px system-ui";
      c.fillText("Undo", 195, 89);
    }
    c.restore();
  }
  private fit() {
    this.dirty = this.leadersDirty = true;
    this.root.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(this.root),
      size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3()),
      tangent = Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2));
    const distance =
      (Math.max(
        size.y / (2 * tangent),
        size.x / (2 * tangent * this.camera.aspect),
      ) +
        size.z / 2) /
      0.78;
    this.orbit.target.copy(center);
    this.camera.position.copy(center).add(new THREE.Vector3(0, 0, distance));
    this.home.copy(this.camera.position);
    this.homeTarget.copy(center);
    this.orbit.update();
  }
  private resize() {
    const rect = this.host.getBoundingClientRect();
    const height = Math.max(1, rect.height);
    this.renderer.setSize(rect.width, height, false);
    this.camera.aspect = rect.width / height;
    this.camera.updateProjectionMatrix();
    this.fit();
  }
  private projectLeaders() {
    const signature = [
      this.camera.position.toArray(),
      this.camera.quaternion.toArray(),
      this.host.clientWidth,
      this.host.clientHeight,
      document.querySelector("#panel")?.scrollTop,
      window.scrollY,
      this.highlightId,
      this.generation,
    ].join("|");
    if (signature === this.leaderSignature) return;
    this.leaderSignature = signature;
    this.lines.replaceChildren();
    if (matchMedia("(max-width: 760px)").matches || !this.data) return;
    const wr = this.workspace.getBoundingClientRect(),
      vr = this.host.getBoundingClientRect();
    this.lines.setAttribute("viewBox", `0 0 ${wr.width} ${wr.height}`);
    const origin = this.camera.getWorldPosition(new THREE.Vector3());
    const list = document.querySelector("#controls")!.getBoundingClientRect();
    for (const [id, target] of this.targets) {
      const row = document.querySelector<HTMLElement>(
        `.control-row[data-id="${CSS.escape(id)}"]`,
      );
      if (!row) continue;
      const rr = row.getBoundingClientRect();
      if (
        rr.top < list.top ||
        rr.bottom > list.bottom ||
        rr.bottom > innerHeight
      )
        continue;
      let p = new THREE.Box3()
        .setFromObject(target)
        .getCenter(new THREE.Vector3());
      const regions = this.data.regions[target.name];
      if (regions) {
        const region =
          Object.entries(regions).find(([, input]) => input === id)?.[0] ??
          "center";
        const offsets: Record<string, [number, number]> = {
          up: [0.5, 0.81],
          down: [0.5, 0.19],
          left: [0.19, 0.5],
          right: [0.81, 0.5],
          center: [0.5, 0.5],
        };
        const [x, y] = offsets[region] ?? offsets.center;
        const box = this.localBounds(target),
          size = box.getSize(new THREE.Vector3());
        p = target.localToWorld(
          new THREE.Vector3(
            box.min.x + size.x * x,
            box.min.y + size.y * y,
            box.min.z + size.z * 0.08,
          ),
        );
      }
      const delta = p.clone().sub(origin);
      this.ray.set(origin, delta.clone().normalize());
      this.ray.far = delta.length() + 0.02;
      const hit = this.ray
        .intersectObjects(this.root.children, true)
        .find((hit) => this.visible(hit.object));
      this.ray.far = Infinity;
      let belongs = false;
      for (let n: THREE.Object3D | null = hit?.object ?? null; n; n = n.parent)
        if (n === target) belongs = true;
      if (!belongs) continue;
      p.project(this.camera);
      const x = vr.left - wr.left + (p.x * 0.5 + 0.5) * vr.width,
        y = vr.top - wr.top + (-p.y * 0.5 + 0.5) * vr.height;
      const endX = rr.left - wr.left,
        endY = rr.top + rr.height / 2 - wr.top;
      const path = document.createElementNS(
        "http://www.w3.org/2000/svg",
        "path",
      );
      path.dataset.input = id;
      path.dataset.x = String(x);
      path.dataset.y = String(y);
      path.setAttribute(
        "d",
        `M${x},${y} L${endX - 18},${endY} L${endX},${endY}`,
      );
      if (row.classList.contains("active")) path.classList.add("active");
      this.lines.append(path);
    }
  }
}
