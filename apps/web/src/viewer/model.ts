import { rendererBackend, rendererURL } from './renderer';
import { badgeStyle, inputIcon, showInputBadge } from '../feedback/input';
import type { Frame, Input, Scene } from '../engine/api';
import type { DetentSound } from '../feedback/detent-sound';
import { DetentFeedback } from '../feedback/detent';
import { RotaryDrag } from './rotary-drag';

export interface ModelData {
  url: string;
  yaw: number;
  physical_width: number;
  optics: { focus_distance: number; aperture: number; focal_length: number; sensor_height: number };
  hidden: string[];
  screen: string;
  inputs: Record<string, string[] | { nodes: string[]; split?: string }>;
  regions: Record<string, Record<string, string>>;
  order: string[];
  credit: { title: string; author: string; url: string; authorUrl: string; license: string };
}
export interface ModelHit {
  id: string;
  point?: { x: number; y: number };
  surfacePoint?: [number, number, number];
}
interface Target {
  x: number;
  y: number;
  visible: boolean;
}
interface Snapshot {
  renderer?: string;
  version?: string;
  ready?: boolean;
  environmentReady?: boolean;
  mode?: 'walk' | 'inspect';
  worldSeconds?: number;
  cameraPosition?: number[];
  cameraRotation?: number[];
  observerPosition?: number[];
  sensorSequence?: number;
  targets?: Record<string, Target>;
  error?: string;
}
interface Rotary {
  id: string;
  negative: string;
  positive: string;
  position: number;
}
type SimulationModule = Pick<typeof import('fujimod/simulation/webgl2'),
  'start' | 'backend' | 'command' | 'snapshot' | 'response' | 'lcd' | 'take_sensor'> & {
  default(): Promise<{ memory: WebAssembly.Memory }>;
};

/** Browser input and DOM overlays. Bevy owns meshes, picking, poses and rendering. */
export class ModelViewer {
  readonly canvas = document.createElement('canvas');
  private lcd = document.createElement('canvas');
  private lcdImage?: ImageData;
  private bootPercent?: number;
  private faulted = false;
  private faultStart?: number;
  private faultImage?: ImageData;
  private data?: ModelData;
  private generation = 0;
  private state: Snapshot = {};
  private pendingMode?: 'walk' | 'inspect';
  private get mode() {
    return this.pendingMode ?? this.state.mode ?? 'inspect';
  }
  private simulation!: SimulationModule;
  private started = false;
  private rendererFailed = false;
  private wasmMemory?: WebAssembly.Memory;
  private ready: Promise<void>;
  private requestID = 0;
  private requests = new Map<number, (hit: ModelHit | null) => void>();
  private held = new Set<number>();
  private orbitPointer?: { id: number; x: number; y: number };
  private orbitEnabled = true;
  private keys = new Set<string>();
  private rotary = new Map<string, Rotary>();
  private rotaryEnabled = true;
  private rotaryDrag: RotaryDrag;
  private rotaryStep?: (id: string, direction: number) => boolean;
  private detent = new DetentFeedback();
  private detentSounds: Record<string, DetentSound> = {};
  private highlightIDs: string[] = [];
  private feedbackID?: string;
  private feedbackUntil = 0;
  private inputMarker = document.createElement('div');
  private inputFeedback = document.createElement('div');
  private modeButton = document.createElement('button');
  private touchMove?: { id: number; x: number; y: number };
  private sceneConsumer?: (scene: Scene) => Promise<void>;
  private publishingScene = false;
  onPointerDown?: (hit: ModelHit, event: PointerEvent) => void;
  onPointerEnd?: (pointer: number) => void;
  onHover?: (id: string | null) => void;
  onRotaryPreview?: (id: string, fraction: number) => number;

  constructor(
    private host: HTMLElement,
    private lines: SVGSVGElement,
    private workspace: HTMLElement,
  ) {
    this.canvas.id = 'simulation-canvas';
    this.canvas.className = 'model-canvas';
    this.canvas.tabIndex = 0;
    this.canvas.setAttribute('aria-label', 'Camera environment');
    host.prepend(this.canvas);
    this.lcd.id = 'lcd-source';
    this.lcd.hidden = true;
    this.lcd.width = 720;
    this.lcd.height = 480;
    host.append(this.lcd);
    this.inputMarker.className = 'model-input-marker';
    this.inputFeedback.className = 'model-input-feedback';
    this.inputFeedback.setAttribute('role', 'status');
    this.inputFeedback.setAttribute('aria-live', 'polite');
    this.inputMarker.hidden = this.inputFeedback.hidden = true;
    this.modeButton.type = 'button';
    this.modeButton.className = 'pill simulation-mode';
    this.modeButton.textContent = 'Walk';
    this.modeButton.onclick = () => this.toggleMode();
    host.append(this.inputMarker, this.inputFeedback, this.modeButton);
    this.rotaryDrag = new RotaryDrag(this.canvas, (enabled) => this.setOrbit(enabled));
    const backend = rendererBackend(location.search);
    const renderer = document.createElement('select');
    renderer.className = 'renderer-backend pill';
    renderer.setAttribute('aria-label', '3D renderer');
    for (const [value, label] of [['webgl2', 'WebGL2'], ['webgpu', 'WebGPU']]) {
      const option = new Option(label, value);
      option.disabled = value === 'webgpu' && !('gpu' in navigator);
      renderer.add(option);
    }
    renderer.value = backend;
    renderer.onchange = () => location.assign(rendererURL(location.href, renderer.value));
    document.querySelector('#view-actions')!.append(renderer);
    this.bindInput();
    this.canvas.addEventListener('webglcontextlost', () => {
      this.showRendererError('3D graphics context was lost. LCD mode is still available.');
    });
    this.canvas.addEventListener('webglcontextcreationerror', (event) => {
      this.showRendererError((event as WebGLContextEvent).statusMessage || 'Unable to create the 3D graphics context.');
    });
    window.addEventListener('simulationerror', (event) => {
      this.showRendererError((event as CustomEvent<string>).detail);
    });
    this.ready = this.start();
    void this.ready.catch((error) => this.showRendererError(error));
  }
  showRendererError(error: unknown) {
    this.rendererFailed = true;
    console.error(error);
    this.host.dataset.rendererError = String(error);
    this.host.dataset.rendererStage = 'failed';
    this.inputFeedback.textContent = String(error);
    this.inputFeedback.classList.add('renderer-error');
    this.inputFeedback.hidden = false;
    this.modeButton.disabled = true;
  }
  private async start() {
    this.host.dataset.rendererStage = 'adapter';
    const backend = rendererBackend(location.search);
    this.host.dataset.rendererBackend = backend;
    if (backend === 'webgpu') {
      const gpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
      if (!gpu || !(await gpu.requestAdapter())) throw Error('WebGPU is unavailable in this browser.');
    } else if (!window.WebGL2RenderingContext) throw Error('3D view requires WebGL 2.');
    this.host.dataset.rendererStage = 'wasm';
    this.simulation = backend === 'webgpu'
      ? await import('fujimod/simulation/webgpu')
      : await import('fujimod/simulation/webgl2');
    this.wasmMemory = (await this.simulation.default()).memory;
    if (this.simulation.backend() !== backend) throw Error('Simulation renderer build mismatch.');
    this.host.dataset.rendererStage = 'engine';
    this.simulation.start('#simulation-canvas', new URL('.', document.baseURI).href);
    this.host.dataset.rendererStage = 'running';
    this.started = true;
    this.host.dataset.renderer = 'bevy';
    this.host.dataset.rendererVersion = '0.20.0';
    requestAnimationFrame(() => this.poll());
    const environment = new URLSearchParams(location.search).get('environment');
    if (environment && /^[a-z0-9-]+$/.test(environment)) {
      const result = await fetch(
        new URL(`environments/${environment}/environment.json`, document.baseURI),
      );
      if (!result.ok) throw Error(`Environment unavailable: ${environment}`);
      const data = await result.json();
      this.send({ type: 'environment', data });
      const credit = document.createElement('a');
      credit.className = 'environment-credit';
      credit.href = data.source;
      credit.textContent = `${data.title} · ${data.author} · CC BY 4.0`;
      this.host.append(credit);
    }
  }
  private send(command: Record<string, unknown>) {
    if (this.rendererFailed) return;
    if (this.started) this.simulation.command(JSON.stringify(command));
    else void this.ready.then(() => this.simulation.command(JSON.stringify(command))).catch(() => {
      // Startup failure is already displayed by showRendererError.
    });
  }
  async load(data: ModelData) {
    const generation = ++this.generation;
    await this.ready;
    if (generation !== this.generation) return;
    this.data = data;
    this.rotary.clear();
    this.send({ type: 'model', data });
    // The scene-ready event includes all glTF dependencies, followed by rig/material setup.
    const started = performance.now();
    await new Promise<void>((resolve, reject) => {
      const check = () => {
        if (generation !== this.generation) {
          resolve();
          return;
        }
        if (this.state.error) {
          reject(Error(this.state.error));
          return;
        }
        if (this.state.ready && performance.now() > started + 50) {
          resolve();
          return;
        }
        if (performance.now() - started > 60000) {
          reject(Error('Bevy camera model did not finish loading.'));
          return;
        }
        requestAnimationFrame(check);
      };
      this.state.ready = false;
      requestAnimationFrame(check);
    });
  }
  private poll() {
    if (this.rendererFailed) return;
    try {
      this.pollFrame();
    } catch (error) {
      this.showRendererError(error);
    }
  }
  private pollFrame() {
    this.host.dataset.wasmMemoryBytes = String(this.wasmMemory?.buffer.byteLength ?? 0);
    const json = this.simulation.snapshot();
    this.state = JSON.parse(json) ?? {};
    if (this.state.mode === this.pendingMode) this.pendingMode = undefined;
    this.host.dataset.simulation = json;
    this.host.dataset.mode = this.state.mode ?? 'inspect';
    this.host.dataset.renderFrames = String(Number(this.host.dataset.renderFrames ?? 0) + 1);
    this.modeButton.textContent = this.mode === 'walk' ? 'Inspect' : 'Walk';
    this.modeButton.setAttribute('aria-pressed', String(this.mode === 'walk'));
    for (const [id, resolve] of this.requests) {
      const result = this.simulation.response(id);
      if (result !== undefined) {
        this.requests.delete(id);
        resolve(JSON.parse(result));
      }
    }
    if (this.faulted && this.faultStart !== undefined) this.paintFault();
    if (this.feedbackID && performance.now() >= this.feedbackUntil) {
      this.feedbackID = undefined;
      this.inputFeedback.hidden = true;
    }
    this.projectOverlays();
    if (this.sceneConsumer && !this.publishingScene && this.state.environmentReady) {
      const rgb = this.simulation.take_sensor();
      if (rgb) {
        this.publishingScene = true;
        void this.sceneConsumer({ rgb, width: 768, height: 512 })
          .catch((error) => {
            this.host.dataset.sceneError = String(error);
          })
          .finally(() => {
            this.publishingScene = false;
          });
      }
    }
    requestAnimationFrame(() => this.poll());
  }
  /** Adapt to an emulator image input; rendering and world state remain together in Rust. */
  setSceneConsumer(consumer?: (scene: Scene) => Promise<void>) {
    this.sceneConsumer = consumer;
    this.send({ type: 'sensor', enabled: !!consumer });
  }
  private bindInput() {
    this.canvas.addEventListener('contextmenu', (event) => {
      event.preventDefault();
      this.toggleMode();
    });
    this.canvas.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return;
      this.canvas.focus({ preventScroll: true });
      this.held.add(event.pointerId);
      if (this.mode === 'walk') {
        this.orbitPointer = { id: event.pointerId, x: event.clientX, y: event.clientY };
        this.canvas.setPointerCapture(event.pointerId);
        if (event.pointerType === 'mouse') this.captureMouse();
        return;
      }
      event.preventDefault();
      void this.pick(event).then((hit) => {
        if (hit && !this.faulted) {
          const part = this.rotary.get(hit.id);
          if (part && this.held.has(event.pointerId)) {
            const gesture = {
              step: (direction: number) => {
                const id = direction < 0 ? part.negative : part.positive;
                if (
                  !this.rotaryEnabled ||
                  !this.rotaryStep?.(id, part.negative === part.positive ? direction : 1)
                )
                  return;
                if (part.negative !== part.positive) this.turnRotary(part.id, direction);
                this.detent.step(this.detentSounds[id]);
                this.highlight(id);
                this.onHover?.(id);
              },
              preview: (fraction: number) =>
                this.previewRotary(
                  part.id,
                  this.onRotaryPreview?.(hit.id, fraction) ?? fraction,
                  true,
                ),
              finish: () => {
                this.onRotaryPreview?.(hit.id, 0);
                this.previewRotary(part.id, 0, false);
              },
            };
            if (this.rotaryDrag.begin(event, gesture)) {
              this.detent.beginGesture(event);
              return;
            }
          }
          this.onPointerDown?.(hit, event);
          if (!this.held.has(event.pointerId)) this.onPointerEnd?.(event.pointerId);
        }
        if (this.held.has(event.pointerId) && this.orbitEnabled && !this.rotaryDrag.dragging) {
          this.orbitPointer = { id: event.pointerId, x: event.clientX, y: event.clientY };
          this.canvas.setPointerCapture(event.pointerId);
        }
      });
    });
    this.canvas.addEventListener('pointermove', (event) => {
      if (document.pointerLockElement === this.canvas) {
        return;
      }
      const drag = this.orbitPointer;
      if (drag?.id === event.pointerId && this.orbitEnabled && !this.rotaryDrag.dragging) {
        this.send({
          type: this.mode === 'walk' ? 'look' : 'orbit',
          x: event.clientX - drag.x,
          y: event.clientY - drag.y,
        });
        drag.x = event.clientX;
        drag.y = event.clientY;
      } else if (!event.buttons && event.pointerType !== 'touch' && this.mode === 'inspect') {
        void this.pick(event).then((hit) => this.onHover?.(hit?.id ?? null));
      }
    });
    const end = (event: PointerEvent) => {
      this.held.delete(event.pointerId);
      if (this.orbitPointer?.id === event.pointerId) this.orbitPointer = undefined;
    };
    for (const type of ['pointerup', 'pointercancel', 'lostpointercapture'])
      window.addEventListener(type, (event) => end(event as PointerEvent));
    this.canvas.addEventListener('pointerleave', () => this.onHover?.(null));
    this.canvas.addEventListener(
      'wheel',
      (event) => {
        if (this.mode !== 'inspect') return;
        event.preventDefault();
        this.send({ type: 'zoom', amount: event.deltaY });
      },
      { passive: false },
    );
    document.addEventListener('mousemove', (event) => {
      // Pointer-lock mousemove remains available when no pointer events are delivered.
      if (document.pointerLockElement === this.canvas && !(event instanceof PointerEvent))
        this.send({ type: 'look', x: event.movementX, y: event.movementY });
    });
    for (const type of ['keydown', 'keyup'] as const)
      window.addEventListener(
        type,
        (event) => {
          if (this.mode !== 'walk' || !['KeyW', 'KeyA', 'KeyS', 'KeyD'].includes(event.code))
            return;
          if (
            event.target instanceof HTMLElement &&
            event.target.closest('input,textarea,select,[contenteditable=true]')
          )
            return;
          event.preventDefault();
          event.stopImmediatePropagation();
          if (type === 'keydown') this.keys.add(event.code);
          else this.keys.delete(event.code);
          this.movement();
        },
        { capture: true },
      );
    const pause = () => {
      this.keys.clear();
      this.held.clear();
      this.orbitPointer = undefined;
      this.touchMove = undefined;
      this.send({ type: 'pause_input' });
      this.rotaryDrag.cancel();
    };
    window.addEventListener('blur', pause);
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) pause();
    });
    document.addEventListener('pointerlockchange', () => {
      if (document.pointerLockElement !== this.canvas) {
        this.keys.clear();
        this.movement();
      }
    });
    this.touchControls();
  }
  private movement() {
    this.send({
      type: 'move',
      right: Number(this.keys.has('KeyD')) - Number(this.keys.has('KeyA')),
      forward: Number(this.keys.has('KeyW')) - Number(this.keys.has('KeyS')),
    });
  }
  private toggleMode() {
    const wasWalking = this.mode === 'walk';
    this.pendingMode = wasWalking ? 'inspect' : 'walk';
    this.keys.clear();
    this.movement();
    this.rotaryDrag.cancel();
    this.orbitPointer = undefined;
    this.send({ type: 'toggle_inspect' });
    if (wasWalking) document.exitPointerLock();
    else if (matchMedia('(pointer:fine)').matches) this.captureMouse();
  }
  private captureMouse() {
    void this.canvas.requestPointerLock().catch(() => {
      // Drag-to-look remains available if the browser declines capture.
      this.canvas.dataset.pointerLock = 'unavailable';
    });
  }
  private touchControls() {
    const stick = document.createElement('div');
    stick.className = 'simulation-stick';
    stick.setAttribute('role', 'group');
    stick.setAttribute('aria-label', 'Walk');
    const nub = document.createElement('span');
    stick.append(nub);
    this.host.append(stick);
    const move = (event: PointerEvent) => {
      if (this.touchMove?.id !== event.pointerId) return;
      const x = Math.max(-1, Math.min(1, (event.clientX - this.touchMove.x) / 36));
      const y = Math.max(-1, Math.min(1, (event.clientY - this.touchMove.y) / 36));
      nub.style.transform = `translate(${x * 24}px,${y * 24}px)`;
      this.send({ type: 'move', right: x, forward: -y });
    };
    stick.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      event.stopPropagation();
      this.touchMove = { id: event.pointerId, x: event.clientX, y: event.clientY };
      stick.setPointerCapture(event.pointerId);
    });
    stick.addEventListener('pointermove', move);
    for (const type of ['pointerup', 'pointercancel', 'lostpointercapture'])
      stick.addEventListener(type, () => {
        this.touchMove = undefined;
        nub.style.transform = '';
        this.send({ type: 'pause_input' });
      });
  }
  isPointerHeld(pointer: number) {
    return this.held.has(pointer);
  }
  private pick(event: PointerEvent): Promise<ModelHit | null> {
    if (!this.data || !this.started) return Promise.resolve(null);
    const rect = this.canvas.getBoundingClientRect(),
      id = ++this.requestID;
    this.send({
      type: 'pick',
      request: id,
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
      radius: event.pointerType === 'touch' ? 12 : 4,
    });
    return new Promise((resolve) => this.requests.set(id, resolve));
  }
  configureRotary(
    inputs: Input[],
    onStep: (id: string, direction: number) => boolean,
    sounds: Record<string, DetentSound>,
  ) {
    this.rotaryDrag.cancel();
    this.rotaryStep = onStep;
    this.detentSounds = sounds;
    const supported = new Map(
      inputs.filter((input) => input.supported).map((input) => [input.id, input]),
    );
    const add = (id: string, negative: string, positive: string) => {
      if (this.rotary.has(id)) return;
      const part = { id, negative, positive, position: 0 };
      for (const alias of [id, negative, positive]) this.rotary.set(alias, part);
      this.send({ type: 'define_rotary', id, aliases: [negative, positive] });
    };
    for (const input of supported.values())
      if (input.kind === 'dial' || input.kind === 'ring') add(input.id, input.id, input.id);
    for (const regions of Object.values(this.data?.regions ?? {})) {
      if (
        regions.up === regions.center &&
        regions.down === regions.center &&
        regions.left !== regions.right &&
        supported.has(regions.left) &&
        supported.has(regions.right)
      )
        add(regions.center, regions.left, regions.right);
    }
    this.setRotaryEnabled(true);
  }
  setRotaryPosition(id: string, index: number) {
    const part = this.rotary.get(id);
    if (!part) return;
    part.position = index;
    this.send({ type: 'rotary', id, position: index, immediate: true });
  }
  turnRotary(id: string, direction: number) {
    const part = this.rotary.get(id);
    if (!part) return;
    part.position += direction;
    this.send({ type: 'rotary', id, position: part.position, immediate: false });
  }
  previewRotary(id: string, fraction: number, held: boolean) {
    const part = this.rotary.get(id);
    if (!part) return;
    this.send({
      type: 'rotary',
      id,
      position: part.position + (held ? fraction : 0),
      immediate: held,
    });
  }
  setRotaryEnabled(enabled: boolean) {
    this.rotaryEnabled = enabled;
    this.rotaryDrag.setEnabled(enabled);
  }
  setOrbit(enabled: boolean) {
    this.orbitEnabled = enabled;
    if (!enabled) this.orbitPointer = undefined;
  }
  faceScreen() {
    this.send({ type: 'face', lens: false });
    document.exitPointerLock();
  }
  faceLens() {
    this.send({ type: 'face', lens: true });
    document.exitPointerLock();
  }
  highlight(value: string | string[] | null) {
    this.highlightIDs = typeof value === 'string' ? [value] : (value ?? []);
    this.send({ type: 'highlight', ids: this.highlightIDs });
  }
  pressFeedback(id: string, _surfacePoint?: [number, number, number]) {
    this.feedbackID = id;
    this.feedbackUntil = performance.now() + badgeStyle.press_ms;
    showInputBadge(this.inputFeedback, id);
    this.inputFeedback.dataset.input = id;
    this.inputFeedback.hidden = false;
  }
  private projectOverlays() {
    const id = this.feedbackID ?? this.highlightIDs[0],
      point = id && this.state.targets?.[id];
    this.inputMarker.hidden = !point || !point.visible || this.host.classList.contains('lcd-mode');
    if (point && id) {
      this.inputMarker.style.left = `${point.x}px`;
      this.inputMarker.style.top = `${point.y}px`;
      this.inputMarker.dataset.input = id;
      const arrow = ['up', 'down', 'left', 'right'].includes(id) ? inputIcon(id) : undefined;
      this.inputMarker.replaceChildren(...(arrow ? [arrow] : []));
      this.inputMarker.classList.toggle('pressed', !!this.feedbackID);
    }
    this.lines.replaceChildren();
    if (matchMedia('(max-width:760px)').matches || this.mode !== 'inspect') return;
    const wr = this.workspace.getBoundingClientRect(),
      vr = this.canvas.getBoundingClientRect();
    this.lines.setAttribute('viewBox', `0 0 ${wr.width} ${wr.height}`);
    const list = document.querySelector('#controls')!.getBoundingClientRect();
    for (const [id, point] of Object.entries(this.state.targets ?? {})) {
      if (!point.visible) continue;
      const row = document.querySelector<HTMLElement>(`.control-row[data-id="${CSS.escape(id)}"]`);
      if (!row) continue;
      const rr = row.getBoundingClientRect();
      if (rr.top < list.top || rr.bottom > list.bottom || rr.bottom > innerHeight) continue;
      const x = vr.left - wr.left + point.x,
        y = vr.top - wr.top + point.y,
        endX = rr.left - wr.left,
        endY = rr.top + rr.height / 2 - wr.top;
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.dataset.input = id;
      path.dataset.x = String(x);
      path.dataset.y = String(y);
      path.setAttribute('d', `M${x},${y} L${endX - 18},${endY} L${endX},${endY}`);
      if (row.classList.contains('active')) path.classList.add('active');
      this.lines.append(path);
    }
  }
  private uploadLCD(image?: ImageData) {
    if (!this.started || this.rendererFailed || this.host.classList.contains('lcd-mode')) return;
    this.simulation.lcd(
      this.lcd.width,
      this.lcd.height,
      new Uint8Array(
        (image ?? this.lcd.getContext('2d')!.getImageData(0, 0, this.lcd.width, this.lcd.height)).data.buffer,
      ),
    );
  }
  view(mode: '3d' | 'lcd') {
    this.host.classList.toggle('lcd-mode', mode === 'lcd');
    this.lcd.hidden = mode !== 'lcd';
    this.lcd.classList.toggle('flat', mode === 'lcd');
    this.send({ type: 'observe', enabled: mode === '3d' });
    if (mode === '3d') this.uploadLCD();
    if (mode === 'lcd') document.exitPointerLock();
  }
  captureView(): string {
    return (this.host.classList.contains('lcd-mode') ? this.lcd : this.canvas).toDataURL(
      'image/png',
    );
  }
  bootProgress(percent?: number) {
    this.bootPercent = percent;
    const context = this.lcd.getContext('2d')!;
    if (percent === undefined) {
      if (this.lcdImage) context.putImageData(this.lcdImage, 0, 0);
      else context.clearRect(0, 0, this.lcd.width, this.lcd.height);
    } else {
      const w = this.lcd.width,
        h = this.lcd.height;
      context.fillStyle = '#101012';
      context.fillRect(0, 0, w, h);
      context.fillStyle = '#d4d4d8';
      context.font = `${w / 30}px system-ui, sans-serif`;
      context.textAlign = 'center';
      context.textBaseline = 'middle';
      context.fillText('It takes a bit to boot a camera', w / 2, h * 0.43);
      context.fillText('in the browser.', w / 2, h * 0.5);
      context.fillStyle = '#343438';
      context.fillRect(w * 0.16, h * 0.59, w * 0.68, h * 0.025);
      context.fillStyle = '#d4d4d8';
      context.fillRect(
        w * 0.16,
        h * 0.59,
        (w * 0.68 * Math.min(100, Math.max(0, percent))) / 100,
        h * 0.025,
      );
    }
    this.uploadLCD();
  }
  frame(frame: Frame) {
    if (frame.rgb.length !== frame.width * frame.height * 3) throw Error('Invalid LCD frame size.');
    if (this.lcd.width !== frame.width || this.lcd.height !== frame.height) {
      this.lcd.width = frame.width;
      this.lcd.height = frame.height;
      this.lcdImage = undefined;
    }
    const context = this.lcd.getContext('2d')!;
    const image = (this.lcdImage ??= context.createImageData(frame.width, frame.height));
    for (let s = 0, t = 0; s < frame.rgb.length; s += 3, t += 4) {
      image.data[t] = frame.rgb[s];
      image.data[t + 1] = frame.rgb[s + 1];
      image.data[t + 2] = frame.rgb[s + 2];
      image.data[t + 3] = 255;
    }
    context.putImageData(image, 0, 0);
    this.uploadLCD(image);

    if (this.bootPercent !== undefined) this.bootProgress(this.bootPercent);
    if (this.faulted) this.paintFault();
    this.host.dataset.frames = String(frame.sequence);
  }
  fault(active: boolean) {
    if (active === this.faulted) return;
    const context = this.lcd.getContext('2d')!;
    this.faulted = active;
    if (active) {
      this.faultImage =
        this.lcdImage ?? context.getImageData(0, 0, this.lcd.width, this.lcd.height);
      this.faultStart = matchMedia('(prefers-reduced-motion: reduce)').matches
        ? undefined
        : performance.now();
      this.paintFault();
    } else {
      const image = this.lcdImage ?? this.faultImage;
      if (image) context.putImageData(image, 0, 0);
      this.faultImage = undefined;
      this.faultStart = undefined;
      this.host.dataset.faultAnimation = 'none';
    }
    this.uploadLCD();
  }
  private paintFault(now = performance.now()) {
    const c = this.lcd.getContext('2d')!,
      w = this.lcd.width,
      h = this.lcd.height,
      scale = (w * 0.8) / 278,
      x = (w - 270 * scale) / 2,
      y = (h - 80 * scale) / 2;
    const progress =
        this.faultStart === undefined ? 1 : Math.min(1, Math.max(0, (now - this.faultStart) / 260)),
      ease = 1 - (1 - progress) ** 3,
      dialogScale = 0.96 + 0.04 * ease,
      image = this.lcdImage ?? this.faultImage;
    // Recompose from the guest frame each time; alpha must never accumulate.
    if (image) c.putImageData(image, 0, 0);
    c.save();
    c.fillStyle = `rgba(0, 0, 0, ${0.32 * ease})`;
    c.fillRect(0, 0, w, h);
    c.globalAlpha = ease;
    c.translate(w / 2, h / 2 + ((8 * w) / 720) * (1 - ease));
    c.scale(dialogScale, dialogScale);
    c.translate(-w / 2, -h / 2);
    c.translate(x, y);
    c.scale(scale, scale);
    c.fillStyle = '#000';
    c.fillRect(-4, -4, 278, 88);
    c.fillStyle = '#fff';
    c.fillRect(-3, -3, 276, 86);
    c.strokeStyle = '#000';
    c.lineWidth = 1;
    c.strokeRect(0.5, 0.5, 269, 79);
    c.fillStyle = '#000';
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
    c.fillStyle = '#fff';
    c.beginPath();
    c.arc(25, 36, 4, 0, Math.PI * 2);
    c.fill();
    c.fillStyle = '#000';
    c.font = '600 16px system-ui';
    c.fillText('Sorry, the emulator', 65, 31);
    c.fillText('stopped.', 65, 51);
    c.restore();
    this.host.dataset.faultAnimation = progress < 1 ? 'entering' : 'settled';
    if (progress === 1) this.faultStart = undefined;
    this.uploadLCD();
  }
}
