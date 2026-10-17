export interface RotaryGesture {
  step: (direction: number) => void;
  preview: (fraction: number) => void;
  finish: () => void;
}

/** A captured pointer belongs to its starting dial until release/cancel. */
export class RotaryDrag {
  private active?: { pointer: number; x: number; y: number; axis?: 'x' | 'y'; remainder: number; gesture: RotaryGesture };
  private swallowed = new Set<number>();
  private enabled = true;
  readonly pixelsPerDetent = 16;

  constructor(private canvas: HTMLCanvasElement, private orbit: (enabled: boolean) => void) {
    canvas.addEventListener('pointermove', event => this.move(event), { capture: true });
    for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) {
      canvas.addEventListener(type, event => this.end(event as PointerEvent), { capture: true });
    }
    window.addEventListener('blur', () => this.cancel());
    document.addEventListener('visibilitychange', () => { if (document.hidden) this.cancel(); });
  }

  get dragging() { return Boolean(this.active); }
  setEnabled(enabled: boolean) { this.enabled = enabled; if (!enabled) this.cancel(); }
  private consume(event: PointerEvent) { event.preventDefault(); event.stopImmediatePropagation(); }

  /** Called in capture, before OrbitControls sees the down event. */
  begin(event: PointerEvent, gesture?: RotaryGesture): boolean {
    if (this.active) {
      // A second finger cannot transfer a dial gesture into a pinch/orbit.
      this.swallowed.add(event.pointerId); this.consume(event);
      this.canvas.setPointerCapture(event.pointerId); return true;
    }
    if (!gesture || event.button !== 0) return false;
    this.consume(event); this.orbit(false);
    this.active = { pointer: event.pointerId, x: event.clientX, y: event.clientY, remainder: 0, gesture };
    if (this.enabled) gesture.preview(0);
    this.canvas.setPointerCapture(event.pointerId); return true;
  }

  private move(event: PointerEvent) {
    if (this.swallowed.has(event.pointerId)) { this.consume(event); return; }
    const active = this.active;
    if (!active || active.pointer !== event.pointerId) return;
    this.consume(event);
    const dx = event.clientX - active.x, dy = active.y - event.clientY;
    // Lock to the first deliberate direction; vertical and horizontal swipes
    // both work when a dial is small or viewed edge-on on a phone.
    if (!active.axis) {
      if (Math.max(Math.abs(dx), Math.abs(dy)) < 4) return;
      active.axis = Math.abs(dx) >= Math.abs(dy) ? 'x' : 'y';
    }
    active.remainder += active.axis === 'x' ? dx : dy;
    active.x = event.clientX; active.y = event.clientY;
    const steps = Math.trunc(active.remainder / this.pixelsPerDetent);
    active.remainder -= steps * this.pixelsPerDetent;
    if (this.enabled) {
      for (let i = 0; i < Math.abs(steps); i++) active.gesture.step(Math.sign(steps));
      active.gesture.preview(active.remainder / this.pixelsPerDetent);
    }
  }

  private end(event: PointerEvent) {
    if (this.swallowed.delete(event.pointerId)) { this.consume(event); return; }
    if (this.active?.pointer !== event.pointerId) return;
    this.consume(event); this.cancel();
  }

  cancel() {
    const active = this.active;
    const pointer = active?.pointer;
    this.active = undefined;
    active?.gesture.finish();
    if (pointer !== undefined && this.canvas.hasPointerCapture(pointer)) this.canvas.releasePointerCapture(pointer);
    // Keep secondary pointers swallowed until their own release.
    this.orbit(true);
  }
}
