import type { Engine, Input } from '../engine/api';
import type { ModelData, ModelHit, ModelViewer } from '../viewer/model';

/** One pointer/keyboard path shared by panel buttons and model parts. */
export class InputPanel {
  private inputs: Input[] = [];
  private positions = new Map<string, string>();
  private holds = new Map<number | string, string>();
  private lastHovered: string | null = null;
  private ready = false;
  private pulseQueue: Promise<void> = Promise.resolve();
  private stopRotation?: () => void;
  constructor(private host: HTMLElement, private viewer: ModelViewer, private error: (error: unknown) => void) {
    viewer.onPointerDown = (hit, event) => this.modelDown(hit, event);
    viewer.onHover = id => this.feedback(id);
    window.addEventListener('pointerup', event => this.release(event.pointerId));
    window.addEventListener('pointercancel', event => this.release(event.pointerId));
    window.addEventListener('blur', () => this.releaseAll());
    document.addEventListener('visibilitychange', () => { if (document.hidden) this.releaseAll(); });
  }
  private engine?: Engine;
  render(inputs: Input[], engine: Engine, model?: ModelData, initial: Record<string, string> = {}) {
    this.releaseAll(); this.engine = engine; this.ready = true;
    this.inputs = [...inputs].sort((a, b) => {
      const order = model?.order ?? [];
      const ai = order.indexOf(a.id), bi = order.indexOf(b.id);
      return (ai < 0 ? 1000 : ai) - (bi < 0 ? 1000 : bi);
    });
    this.host.replaceChildren(); this.positions.clear();
    for (const input of this.inputs) {
      const row = document.createElement('div'); row.className = 'control-row'; row.dataset.id = input.id;
      const label = model?.labels[input.id] ?? input.label;
      const text = document.createElement('span'); text.textContent = label; row.append(text);
      if (!input.supported) {
        const disabled = document.createElement('button'); disabled.disabled = true; disabled.textContent = 'Not modelled'; row.append(disabled);
      } else if (input.positions?.length) {
        const select = document.createElement('select'); select.setAttribute('aria-label', label);
        for (const position of input.positions) select.add(new Option(position.label, position.id));
        const selected = initial[input.id] ?? input.initial ?? input.positions[0].id;
        select.value = selected; this.positions.set(input.id, selected);
        select.onchange = () => this.position(input, select.value); row.append(select);
      } else if (input.relative) {
        for (const [symbol, step] of [['−', -1], ['+', 1]] as const) {
          const button = document.createElement('button'); button.textContent = symbol; button.setAttribute('aria-label', `${label} ${step < 0 ? 'left' : 'right'}`);
          button.onclick = () => { this.feedback(input.id); void engine.setPosition(input.id, step).catch(this.error); }; row.append(button);
        }
      } else {
        const stages = input.stages ?? [{ id: '', label }];
        if (!input.stages) text.remove();
        for (const stage of stages) {
          const button = document.createElement('button'); button.textContent = stage.id ? stage.id === 'half' ? 'Half' : 'Full' : label;
          button.setAttribute('aria-label', stage.id ? `${label} ${stage.id}` : label);
          button.addEventListener('pointerdown', event => {
            event.preventDefault(); button.setPointerCapture(event.pointerId);
            this.press(input, event.pointerId, stage.id || undefined);
          });
          // Native click covers keyboard and assistive technology; pointer taps are handled above.
          button.addEventListener('click', event => {
            if (event.detail !== 0) return;
            const key = `keyboard-${input.id}`; this.press(input, key, stage.id || undefined); this.release(key);
          }); row.append(button);
        }
      }
      row.addEventListener('pointerenter', () => this.feedback(input.id));
      row.addEventListener('pointerleave', () => { if (!this.holds.size) this.feedback(null); });
      if (input.kind === 'dial' || input.kind === 'ring') row.addEventListener('pointerdown', event => {
        if (event.target === row || event.target === text) this.rotate(input, event);
      });
      this.host.append(row);
    }
    this.viewer.configureRotary(this.inputs, (id, direction) => {
      const input = this.inputs.find(input => input.id === id);
      return input ? this.step(input, direction) : false;
    });
    this.viewer.setRotaryEnabled(true);
  }
  disable() { this.viewer.setRotaryEnabled(false); this.releaseAll(); this.ready = false; for (const control of this.host.querySelectorAll<HTMLButtonElement | HTMLSelectElement>('button,select')) control.disabled = true; }
  resume() {
    this.ready = true; this.viewer.setRotaryEnabled(true);
    for (const row of this.host.querySelectorAll<HTMLElement>('.control-row')) {
      const supported = this.inputs.find(input => input.id === row.dataset.id)?.supported;
      for (const control of row.querySelectorAll<HTMLButtonElement | HTMLSelectElement>('button,select')) control.disabled = !supported;
    }
  }
  clear() { this.disable(); this.host.replaceChildren(); this.inputs = []; this.engine = undefined; }
  private feedback(id: string | null) {
    this.lastHovered = id;
    for (const row of this.host.querySelectorAll<HTMLElement>('.control-row')) row.classList.toggle('active', row.dataset.id === id);
    this.viewer.highlight(id);
  }
  private press(input: Input, pointer: number | string, stage?: string, point?: { x: number; y: number }) {
    if (!this.ready || !input.supported || [...this.holds.values()].includes(input.id)) return;
    this.holds.set(pointer, input.id); this.feedback(input.id);
    void this.engine!.press(input.id, stage, point).catch(this.error);
  }
  private release(pointer: number | string) {
    const id = this.holds.get(pointer); if (!id) return;
    this.holds.delete(pointer);
    void this.engine?.release(id).catch(this.error);
    this.viewer.setOrbit(true);
    // Keep a short visible response even to a single-frame tap.
    window.setTimeout(() => { if (!this.holds.size && this.lastHovered === id) this.feedback(null); }, 160);
  }
  private releaseAll() { this.stopRotation?.(); for (const key of [...this.holds.keys()]) this.release(key); }
  private position(input: Input, position: string) {
    this.positions.set(input.id, position); this.feedback(input.id);
    const select = this.host.querySelector<HTMLSelectElement>(`[data-id="${CSS.escape(input.id)}"] select`);
    if (select) select.value = position;
    void this.engine!.setPosition(input.id, position).catch(this.error);
  }
  private step(input: Input, direction: number): boolean {
    if (!this.ready || !input.supported || !this.engine || !direction) return false;
    if (input.relative) {
      void this.engine.setPosition(input.id, Math.sign(direction)).catch(this.error);
      return true;
    }
    const positions = input.positions;
    if (positions?.length) {
      const current = positions.findIndex(p => p.id === this.positions.get(input.id));
      if (current < 0) return false;
      const next = Math.max(0, Math.min(positions.length - 1, current + Math.sign(direction)));
      if (next === current) return false;
      this.position(input, positions[next].id);
      return true;
    }
    // Rear command detents are physical button actions selected by the viewer.
    const engine = this.engine, id = input.id;
    this.pulseQueue = this.pulseQueue.then(async () => {
      await engine.press(id);
      await engine.release(id);
    }).catch(this.error);
    return true;
  }
  private rotate(input: Input, event: PointerEvent) {
    if (!this.ready || !input.supported) return;
    event.preventDefault(); event.stopPropagation(); this.viewer.setOrbit(false); this.feedback(input.id);
    let previous = event.clientX, changed = false;
    const move = (next: PointerEvent) => {
      if (next.pointerId !== event.pointerId) return;
      const delta = next.clientX - previous;
      if (Math.abs(delta) >= 16) { this.step(input, Math.sign(delta)); previous = next.clientX; changed = true; }
    };
    const end = (up: PointerEvent) => {
      if (up.pointerId !== event.pointerId) return;
      if (!changed && up.type !== 'pointercancel') this.step(input, 1);
      cleanup();
    };
    const cleanup = () => { this.viewer.setOrbit(true); window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', end); window.removeEventListener('pointercancel', end); this.stopRotation = undefined; };
    this.stopRotation = cleanup;
    window.addEventListener('pointermove', move); window.addEventListener('pointerup', end); window.addEventListener('pointercancel', end);
  }
  private modelDown(hit: ModelHit, event: PointerEvent) {
    const input = this.inputs.find(input => input.id === hit.id);
    if (!input?.supported || !this.ready || input.kind === 'dial' || input.kind === 'ring') return;
    event.preventDefault(); event.stopPropagation(); this.viewer.setOrbit(false);
    this.viewer.renderer.domElement.setPointerCapture(event.pointerId); this.feedback(input.id);
    if (input.positions?.length) { this.step(input, 1); this.viewer.setOrbit(true); }
    else this.press(input, event.pointerId, input.stages ? 'full' : undefined, hit.point);
  }
}
