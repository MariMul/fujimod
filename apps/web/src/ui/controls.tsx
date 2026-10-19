import { For, Show, Switch, Match, createSignal, createEffect, onSettled, untrack } from 'solid-js';
import { inputName } from '../feedback/input';
import type { Engine, Input } from '../engine/api';
import type { ModelHit, ModelViewer } from '../viewer/model';
import type { ControlNode, DevicePanel, PowerCombo } from './layout';
import { Icon } from './icons';
import type { DetentSound } from '../feedback/detent-sound';
import { DetentFeedback } from '../feedback/detent';

/** Input protocol and gesture lifetime; Solid owns the panel's rendered state. */
export class ControlSession {
  private revision;
  changed;
  read() {
    this.revision();
    return this;
  }
  data?: DevicePanel;
  detent = new DetentFeedback();
  detentSounds: Record<string, DetentSound> = {};
  inputs = new Map<string, Input>();
  relativeSteps = new Map<string, number>();
  modelFractions = new Map<string, number>();
  positions = new Map<string, string>();
  renders = new Map<string, () => void>();
  holds = new Map<number | string, string>();
  engine?: Engine;
  ready = false;
  powered = true;
  powerID?: string;
  active: string[] = [];
  glowing = false;
  stopShutter?: (release?: boolean) => void;
  stopRotation?: () => void;
  rotation?: { dial: HTMLElement; fraction: number; center?: number };
  private pulseQueue: Promise<unknown> = Promise.resolve();
  constructor(
    readonly viewer: ModelViewer,
    readonly error: (error: unknown) => void,
    readonly combo: (combo: PowerCombo) => Promise<void>,
  ) {
    const [revision, setRevision] = createSignal(0);
    this.revision = revision;
    this.changed = () => setRevision((n) => n + 1);
    viewer.onRotaryPreview = (id, fraction) => {
      fraction = this.clampFraction(id, fraction);
      this.modelFractions.set(id, fraction);
      this.changed();
      this.renders.get(id)?.();
      return fraction;
    };
    viewer.onPointerEnd = (pointer) => this.release(pointer);
    viewer.onPointerDown = (hit, event) => this.modelDown(hit, event);
    viewer.onHover = (id) => this.feedback(id ? [id] : []);
  }
  listen() {
    const release = (event: PointerEvent) => this.release(event.pointerId);
    const blur = () => this.releaseAll();
    const visibility = () => {
      if (document.hidden) this.releaseAll();
    };
    window.addEventListener('pointerup', release);
    window.addEventListener('pointercancel', release);
    window.addEventListener('blur', blur);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      this.clear();
      window.removeEventListener('pointerup', release);
      window.removeEventListener('pointercancel', release);
      window.removeEventListener('blur', blur);
      document.removeEventListener('visibilitychange', visibility);
    };
  }
  render(inputs: Input[], engine: Engine, data: DevicePanel, initial: Record<string, string> = {}) {
    this.clear();
    this.engine = engine;
    this.detentSounds = data.detentSounds ?? {};
    this.ready = true;
    this.inputs = new Map(
      inputs.map((input) => [input.id, { ...input, label: inputName(input.id) }]),
    );
    for (const input of inputs)
      if (input.positions?.length)
        this.positions.set(input.id, initial[input.id] ?? input.initial ?? input.positions[0].id);
    const visit = (nodes: ControlNode[]) => {
      for (const node of nodes) {
        if (node.operation === 'power') this.powerID = node.id;
        if (node.variant === 'segment' && node.id && !this.positions.has(node.id)) {
          const first =
            this.inputs.get(node.id)?.positions?.[0]?.id ?? Object.keys(node.labels ?? {})[0];
          if (first !== undefined) this.positions.set(node.id, first);
        }
        visit(node.children ?? []);
      }
    };
    visit(data.layout);
    this.data = data;
    this.changed();
    this.viewer.configureRotary(
      inputs,
      (id, direction) => this.step(id, direction),
      this.detentSounds,
    );
    for (const id of this.positions.keys()) this.syncRotary(id);
    this.viewer.setRotaryEnabled(true);
  }
  index(id: string) {
    return Math.max(
      0,
      this.inputs.get(id)?.positions?.findIndex((p) => p.id === this.positions.get(id)) ?? 0,
    );
  }
  disabled(id: string) {
    this.read();
    return !this.ready || !this.inputs.get(id)?.supported || (!this.powered && id !== this.powerID);
  }
  classes(id: string) {
    this.read();
    return {
      active: this.active.includes(id) && !this.glowing,
      glow: this.active.includes(id) && this.glowing,
    };
  }
  shutterDown(
    id: string,
    tray: HTMLElement,
    state: (stage: string, left: string) => void,
    event: PointerEvent,
  ) {
    if (!this.ready || !this.powered) return;
    event.preventDefault();
    tray.setPointerCapture(event.pointerId);
    const started = performance.now(),
      bounds = tray.getBoundingClientRect(),
      origin = event.clientX,
      engine = this.engine!;
    let full = false,
      moved = false;
    const stage = (value: 'half' | 'full') => {
      full = value === 'full';
      state(value, '');
      void engine.press(id, value).catch((error) => {
        if (this.engine === engine && this.ready) this.error(error);
      });
    };
    stage('half');
    const move = (next: PointerEvent) => {
      if (next.pointerId !== event.pointerId) return;
      const delta = next.clientX - origin;
      moved ||= Math.abs(delta) > 5;
      const travel = Math.max(1, bounds.width - 70);
      if (delta >= travel && !full) stage('full');
      else if (delta < travel * 0.8 && full) stage('half');
      state(full ? 'full' : 'half', full ? '' : `${3 + Math.max(0, Math.min(travel, delta))}px`);
    };
    const cleanup = (release = false) => {
      if (release) {
        const engine = this.engine;
        void engine?.release(id).catch((error) => {
          if (this.engine === engine && this.ready) this.error(error);
        });
      }
      state('idle', '');
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', finish);
      window.removeEventListener('pointercancel', finish);
      this.stopShutter = undefined;
    };
    const finish = (up: PointerEvent) => {
      if (up.pointerId !== event.pointerId) return;
      if (up.type !== 'pointercancel' && !moved && !full && performance.now() - started < 250)
        stage('full');
      const engine = this.engine!;
      void engine.release(id).catch((error) => {
        if (this.engine === engine && this.ready) this.error(error);
      });
      cleanup();
    };
    this.stopShutter = cleanup;
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', finish);
    window.addEventListener('pointercancel', finish);
  }
  shutterKey(id: string, event: KeyboardEvent) {
    if ((event.key === ' ' || event.key === 'Enter') && !event.repeat) {
      event.preventDefault();
      if (this.ready && this.powered) {
        const engine = this.engine!;
        void engine
          .press(id, 'half')
          .then(() => engine.press(id, 'full'))
          .then(() => engine.release(id))
          .catch((error) => {
            if (this.engine === engine && this.ready) this.error(error);
          });
      }
    }
  }
  private clampFraction(id: string, fraction: number) {
    const positions = this.inputs.get(id)?.positions;
    if (!positions?.length) return fraction;
    const index = positions.findIndex((p) => p.id === this.positions.get(id));
    return Math.max(-index, Math.min(positions.length - 1 - index, fraction));
  }
  private syncRotary(id: string) {
    const input = this.inputs.get(id);
    if (!input?.positions?.length) return;
    const index = input.positions.findIndex((p) => p.id === this.positions.get(id));
    const initial = input.positions.findIndex(
      (p) => p.id === (input.initial ?? input.positions![0].id),
    );
    if (index >= 0) this.viewer.setRotaryPosition(id, index - Math.max(0, initial));
  }
  position(id: string, value: string) {
    if (
      !this.ready ||
      (!this.powered && id !== this.powerID) ||
      !this.inputs.get(id)?.supported ||
      this.positions.get(id) === value
    )
      return;
    this.positions.set(id, value);
    this.changed();
    this.syncRotary(id);
    this.renders.get(id)?.();
    this.feedback([id]);
    const engine = this.engine!;
    if (id === this.powerID) {
      this.powered = value === 'on';
      if (this.powered) this.ready = false;
      this.availability();
    }
    void engine
      .setPosition(id, value)
      .then(() => {
        if (this.engine === engine && id === this.powerID && value === 'on') {
          this.ready = true;
          this.availability();
        }
      })
      .catch((error) => {
        if (this.engine === engine) this.error(error);
      });
  }
  step(id: string, direction: number): boolean {
    const input = this.inputs.get(id);
    if (!this.ready || !this.powered || !input?.supported) return false;
    if (input.relative) {
      this.relativeSteps.set(id, (this.relativeSteps.get(id) ?? 0) + Math.sign(direction));
      this.changed();
      this.viewer.turnRotary(id, Math.sign(direction));
      void this.engine!.setPosition(id, Math.sign(direction)).catch(this.error);
      return true;
    }
    if (input.positions?.length) {
      const current = input.positions.findIndex((p) => p.id === this.positions.get(id));
      const next = Math.max(
        0,
        Math.min(input.positions.length - 1, current + Math.sign(direction)),
      );
      if (next === current) return false;
      this.position(id, input.positions[next].id);
      return true;
    }
    void this.pulse(id);
    return true;
  }
  pulse(id: string) {
    const engine = this.engine!,
      task = this.pulseQueue
        .then(() => {
          this.viewer.pressFeedback(id);
          return engine.press(id);
        })
        .then(() => engine.release(id));
    this.pulseQueue = task.catch((error) => {
      if (this.engine === engine && this.ready) this.error(error);
    });
    return task;
  }
  press(
    id: string,
    pointer: number | string,
    stage?: string,
    point?: { x: number; y: number },
    surfacePoint?: ModelHit['surfacePoint'],
  ) {
    if (
      !this.ready ||
      !this.powered ||
      !this.inputs.get(id)?.supported ||
      [...this.holds.values()].includes(id)
    )
      return;
    this.holds.set(pointer, id);
    this.feedback([id]);
    this.viewer.pressFeedback(id, this.inputs.get(id)?.kind === 'touch' ? surfacePoint : undefined);
    void this.engine!.press(id, stage, point).catch(this.error);
  }
  release(pointer: number | string) {
    const id = this.holds.get(pointer);
    if (!id) return;
    this.holds.delete(pointer);
    const engine = this.engine;
    void engine?.release(id).catch((error) => {
      if (this.engine === engine && this.ready) this.error(error);
    });
    this.viewer.setOrbit(true);
    setTimeout(() => {
      if (!this.holds.size) this.feedback([]);
    }, 160);
  }
  releaseAll() {
    this.stopRotation?.();
    this.stopShutter?.(true);
    for (const key of [...this.holds.keys()]) this.release(key);
  }
  feedback(ids: string[], glow = false) {
    this.active = ids;
    this.glowing = glow;
    this.changed();
    this.viewer.highlight(ids);
  }
  rotate(id: string, dial: HTMLElement, event: PointerEvent) {
    if (event.button !== 0 || !this.ready || !this.powered || !this.inputs.get(id)?.supported)
      return;
    if (this.stopRotation) return;
    this.detent.beginGesture(event);
    event.preventDefault();
    dial.setPointerCapture(event.pointerId);
    this.viewer.setOrbit(false);
    const values = dial.querySelector<HTMLElement>('.vals')!;
    const ticks = dial.classList.contains('ticks');
    const initialTicks = parseFloat(values.style.backgroundPositionX) || 0;
    const centers = Array.from(values.children, (value) => {
      const label = value as HTMLElement;
      return label.offsetLeft + label.offsetWidth / 2;
    });
    const rotation: NonNullable<ControlSession['rotation']> = { dial, fraction: 0 };
    if (centers.length) {
      // Start from the rendered position, even when a previous snap is in flight.
      const transform = getComputedStyle(values).transform;
      const x = new DOMMatrixReadOnly(transform).m41;
      rotation.center = dial.clientWidth / 2 - x;
      values.style.transform = `translateX(${x}px)`;
    }
    this.rotation = rotation;
    values.style.transition = 'none';
    let previous = event.clientX;
    let remainder = 0;
    let tickPosition = initialTicks;
    const move = (next: PointerEvent) => {
      if (next.pointerId !== event.pointerId) return;
      const delta = next.clientX - previous;
      previous = next.clientX;
      if (rotation.center !== undefined) {
        // Measure travel in CSS pixels, using the actual, uneven label spacing.
        const center = Math.max(centers[0], Math.min(centers.at(-1)!, rotation.center - delta));
        rotation.center = center;
        const nearest = centers.reduce(
          (best, candidate, index) =>
            Math.abs(candidate - center) < Math.abs(centers[best] - center) ? index : best,
          0,
        );
        const current = this.index(id);
        const direction = Math.sign(nearest - current);
        for (let n = 0; n < Math.abs(nearest - current); n++) {
          if (this.step(id, direction)) this.detent.step(this.detentSounds[id]);
        }
        const index = this.index(id);
        const distance = center - centers[index];
        const neighbor = centers[index + Math.sign(distance)];
        rotation.fraction =
          neighbor === undefined || distance === 0
            ? 0
            : distance / Math.abs(neighbor - centers[index]);
      } else {
        remainder += delta * (ticks ? 1 : -1);
        const steps = Math.trunc(remainder / 16);
        remainder -= steps * 16;
        for (let n = 0; n < Math.abs(steps); n++) {
          if (this.step(id, Math.sign(steps))) {
            tickPosition += Math.sign(steps) * 16;
            this.detent.step(this.detentSounds[id]);
          }
        }
        rotation.fraction = this.clampFraction(id, remainder / 16);
      }
      this.changed();
      this.viewer.previewRotary(id, rotation.fraction, true);
      if (ticks) values.style.backgroundPositionX = `${tickPosition + remainder}px`;
      else this.renders.get(id)?.();
    };
    const end = (up: PointerEvent) => {
      if (up.pointerId === event.pointerId) cleanup();
    };
    const cleanup = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', end);
      dial.removeEventListener('lostpointercapture', end);
      this.rotation = undefined;
      this.changed();
      this.viewer.previewRotary(id, 0, false);
      values.style.transition = ticks
        ? 'background-position-x 140ms ease-out'
        : 'transform 140ms ease-out';
      if (ticks) values.style.backgroundPositionX = `${tickPosition}px`;
      else this.renders.get(id)?.();
      if (dial.hasPointerCapture(event.pointerId)) dial.releasePointerCapture(event.pointerId);
      this.viewer.setOrbit(true);
      this.stopRotation = undefined;
    };
    this.stopRotation = cleanup;
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
    dial.addEventListener('lostpointercapture', end);
  }
  private modelDown(hit: ModelHit, event: PointerEvent) {
    const input = this.inputs.get(hit.id);
    if (!input?.supported || !this.ready || input.kind === 'dial' || input.kind === 'ring') return;
    event.preventDefault();
    event.stopPropagation();
    this.viewer.setOrbit(false);
    if (this.viewer.isPointerHeld(event.pointerId))
      this.viewer.canvas.setPointerCapture(event.pointerId);
    if (input.positions?.length) this.step(hit.id, 1);
    else
      this.press(
        hit.id,
        event.pointerId,
        input.stages ? 'full' : undefined,
        hit.point,
        hit.surfacePoint,
      );
  }
  private availability() {
    this.changed();
    this.viewer.setRotaryEnabled(this.ready && this.powered);
  }
  disable() {
    this.ready = false;
    this.holds.clear();
    this.stopRotation?.();
    this.stopShutter?.();
    this.feedback([]);
    this.availability();
  }
  resume() {
    this.ready = true;
    this.powered = true;
    if (this.powerID) {
      this.positions.set(this.powerID, 'on');
      this.changed();
      this.renders.get(this.powerID)?.();
    }
    this.availability();
  }
  synchronize(positions: Record<string, string>) {
    for (const [id, value] of Object.entries(positions)) {
      this.positions.set(id, value);
      this.changed();
      this.syncRotary(id);
      this.renders.get(id)?.();
    }
    if (this.powerID && positions[this.powerID]) this.powered = positions[this.powerID] === 'on';
    this.availability();
  }
  clear() {
    this.releaseAll();
    this.ready = false;
    this.stopShutter?.();
    this.powered = true;
    this.powerID = undefined;
    this.viewer.setRotaryEnabled(false);
    this.data = undefined;
    this.changed();
    this.inputs.clear();
    this.detentSounds = {};
    this.positions.clear();
    this.modelFractions.clear();
    this.relativeSteps.clear();

    this.renders.clear();
    this.engine = undefined;
  }
}

function Key(props: {
  session: ControlSession;
  id: string;
  node: Pick<ControlNode, 'face' | 'icon' | 'hint' | 'variant'>;
  class?: string;
}) {
  return (
    <button
      type="button"
      class={[
        'key',
        props.class,
        { rec: props.node.variant === 'record', wifi: !!props.node.hint },
        props.session.classes(props.id),
      ]}
      data-id={props.id}
      aria-label={props.session.inputs.get(props.id)?.label ?? props.id}
      disabled={props.session.disabled(props.id)}
      aria-disabled={props.session.disabled(props.id) ? 'true' : 'false'}
      onPointerDown={(event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        props.session.press(props.id, event.pointerId);
      }}
      onClick={(event) => {
        if (event.detail === 0 && !props.session.disabled(props.id))
          void props.session.pulse(props.id);
      }}
      onPointerEnter={() => props.session.feedback([props.id])}
      onPointerLeave={() => {
        if (!props.session.holds.size) props.session.feedback([]);
      }}
    >
      <Show
        when={props.node.variant === 'record'}
        fallback={
          <Show
            when={props.node.icon}
            fallback={
              <span class={{ two: (props.node.face?.length ?? 0) > 1 }}>
                {props.node.face?.join('\n') ??
                  props.session.inputs.get(props.id)?.label ??
                  props.id}
              </span>
            }
          >
            <Icon name={props.node.icon!} />
          </Show>
        }
      >
        <i />
      </Show>
      <Show when={props.node.hint}>
        <em>{props.node.hint}</em>
      </Show>
    </button>
  );
}
function Shutter(props: { session: ControlSession; id: string }) {
  const [stage, setStage] = createSignal('idle'),
    [left, setLeft] = createSignal('');
  const state = (value: string, offset: string) => {
    setStage(value);
    setLeft(offset);
  };
  return (
    <div
      class={['slide', stage(), props.session.classes(props.id)]}
      tabindex="0"
      data-id={props.id}
      role="button"
      aria-label={props.session.inputs.get(props.id)?.label ?? props.id}
      aria-disabled={props.session.disabled(props.id) ? 'true' : 'false'}
      onPointerDown={(event) =>
        props.session.shutterDown(props.id, event.currentTarget, state, event)
      }
      onKeyDown={(event) => props.session.shutterKey(props.id, event)}
    >
      <span class="thumb" style={{ left: left() }}>
        {stage() === 'idle' ? '' : stage().toUpperCase()}
      </span>
      <span class="end">FULL</span>
    </div>
  );
}
function Dial(props: { session: ControlSession; id: string; node: ControlNode }) {
  let dial!: HTMLDivElement, values!: HTMLDivElement;
  const positions = untrack(() => props.session.inputs.get(props.id)?.positions ?? []);
  const selected = () => props.session.read().positions.get(props.id);
  let pending = false;
  // Pointer geometry belongs to this ref; selected values and availability belong to JSX.
  const update = () => {
    if (pending || !dial) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      if (!dial.isConnected || props.node.variant === 'ticks') return;
      const labels = [...values.children] as HTMLElement[];
      for (const label of labels)
        if (!label.style.width) {
          const wasSelected = label.classList.contains('on');
          label.classList.add('on');
          label.style.width = `${label.offsetWidth}px`;
          label.style.flexShrink = '0';
          label.classList.toggle('on', wasSelected);
        }
      const rotation = props.session.rotation;
      if (rotation?.dial === dial && rotation.center !== undefined) {
        values.style.transform = `translateX(${dial.clientWidth / 2 - rotation.center}px)`;
        return;
      }
      const fraction =
        rotation?.dial === dial
          ? rotation.fraction
          : (props.session.modelFractions.get(props.id) ?? 0);
      const visual = Math.max(
        0,
        Math.min(positions.length - 1, props.session.index(props.id) + fraction),
      );
      const a = labels[Math.floor(visual)],
        b = labels[Math.ceil(visual)];
      if (a && b) {
        const center = (item: HTMLElement) => item.offsetLeft + item.offsetWidth / 2;
        const offset = center(a) + (center(b) - center(a)) * (visual % 1);
        values.style.transform = `translateX(${dial.clientWidth / 2 - offset}px)`;
      }
    });
  };
  createEffect(() => {
    props.session.read();
    return selected();
  }, update);
  onSettled(() => {
    props.session.renders.set(props.id, update);
    const down = (event: PointerEvent) => props.session.rotate(props.id, dial, event);
    dial.addEventListener('pointerdown', down);
    const observer = new ResizeObserver(update);
    observer.observe(dial);
    update();
    return () => {
      dial.removeEventListener('pointerdown', down);
      observer.disconnect();
      props.session.renders.delete(props.id);
    };
  });
  return (
    <div
      ref={dial}
      class={['dial', { ticks: props.node.variant === 'ticks' }, props.session.classes(props.id)]}
      tabindex="0"
      role="slider"
      aria-label={props.session.inputs.get(props.id)?.label ?? props.node.caption ?? props.id}
      data-id={props.id}
      aria-disabled={props.session.disabled(props.id) ? 'true' : 'false'}
      aria-valuenow={props.session.read().index(props.id)}
      aria-valuemin="0"
      aria-valuemax={Math.max(0, positions.length - 1)}
      aria-valuetext={selected() ?? ''}
      onKeyDown={(event) => {
        if (!['ArrowLeft', 'ArrowDown', 'ArrowRight', 'ArrowUp'].includes(event.key)) return;
        event.preventDefault();
        props.session.detent.beginGesture(event);
        if (props.session.step(props.id, ['ArrowLeft', 'ArrowDown'].includes(event.key) ? -1 : 1))
          props.session.detent.step(props.session.detentSounds[props.id]);
      }}
    >
      <div
        ref={values}
        class="vals"
        style={{
          'background-position-x':
            props.node.variant === 'ticks'
              ? `${((props.session.read().relativeSteps.get(props.id) ?? 0) + (props.session.read().rotation?.dial === dial ? props.session.read().rotation!.fraction : (props.session.read().modelFractions.get(props.id) ?? 0))) * 16}px`
              : undefined,
        }}
      >
        <For each={positions}>
          {(position) => (
            <span data-value={position.id} class={{ on: position.id === selected() }}>
              {props.node.labels?.[position.id] ?? position.label}
            </span>
          )}
        </For>
      </div>
    </div>
  );
}
function Node(props: { session: ControlSession; node: ControlNode }) {
  const node = untrack(() => props.node),
    id = node.id ?? '';
  const input = untrack(() => props.session.inputs.get(id));
  const positions =
    input?.positions ?? Object.keys(node.labels ?? {}).map((id) => ({ id, label: id }));
  const keys =
    node.kind === 'command'
      ? [
          ['left', '◀', 'side l'],
          ['center', '', 'tab'],
          ['right', '▶', 'side r'],
        ]
      : [
          ['up', '▲', 'a u'],
          ['down', '▼', 'a d'],
          ['left', '◀', 'a l'],
          ['right', '▶', 'a r'],
          ['center', node.face?.join('\n') ?? '', 'ok'],
        ];
  return (
    <Switch>
      <Match when={node.kind === 'divider'}>
        <hr />
      </Match>
      <Match when={node.kind === 'space'}>
        <div class="sp" />
      </Match>
      <Match when={node.kind === 'command' || node.kind === 'dpad'}>
        <div class={node.kind === 'command' ? 'cmd' : 'dpad'}>
          <Show when={node.kind === 'command'}>
            <div class="cap">{node.caption}</div>
          </Show>
          <div
            class={node.kind === 'command' ? 'knurl' : 'dpad-keys'}
            style={node.kind === 'dpad' ? { display: 'contents' } : {}}
          >
            <For each={keys}>
              {(key) => (
                <Key
                  session={props.session}
                  id={node.bindings![key[0]]}
                  node={{ face: key[1].split('\n') }}
                  class={key[2]}
                />
              )}
            </For>
          </div>
        </div>
      </Match>
      <Match when={node.kind !== 'control'}>
        <div class={node.className ?? ''}>
          <For each={node.children ?? []}>
            {(child) => <Node node={child} session={props.session} />}
          </For>
        </div>
      </Match>
      <Match when={node.variant === 'key' || node.variant === 'record'}>
        <Show
          when={node.caption}
          fallback={<Key session={props.session} id={id} node={node} class={node.className} />}
        >
          <div class={`control-node ${node.className ?? ''}`}>
            <div class="cap">{node.caption}</div>
            <Key session={props.session} id={id} node={node} class={node.className} />
          </div>
        </Show>
      </Match>
      <Match when={true}>
        <div class={`control-node ${node.className ?? ''}`} data-id={id}>
          <Show when={node.caption}>
            <div class="cap">{node.caption}</div>
          </Show>
          <Switch>
            <Match when={node.variant === 'segment'}>
              <div class="seg wide" role="group" aria-label={input?.label ?? node.caption ?? id}>
                <For each={positions}>
                  {(position) => (
                    <button
                      type="button"
                      data-value={position.id}
                      class={[
                        '',
                        { on: props.session.read().positions.get(id) === position.id },
                        props.session.classes(id),
                      ]}
                      aria-pressed={
                        props.session.read().positions.get(id) === position.id ? 'true' : 'false'
                      }
                      aria-label={`${input?.label ?? node.caption} ${node.labels?.[position.id] ?? position.label}`}
                      disabled={props.session.disabled(id)}
                      aria-disabled={props.session.disabled(id) ? 'true' : 'false'}
                      onClick={() => props.session.position(id, position.id)}
                    >
                      {node.labels?.[position.id] ?? position.label}
                    </button>
                  )}
                </For>
              </div>
            </Match>
            <Match when={node.variant === 'shutter'}>
              <Shutter session={props.session} id={id} />
            </Match>
            <Match when={true}>
              <Dial session={props.session} id={id} node={node} />
            </Match>
          </Switch>
        </div>
      </Match>
    </Switch>
  );
}
function Combo(props: { session: ControlSession; item: PowerCombo }) {
  const [held, setHeld] = createSignal(false);
  const glow = () =>
    props.session.feedback(
      props.item.inputs.map((input) => input.id),
      true,
    );
  return (
    <button
      class={['combo', { held: held() }]}
      type="button"
      aria-label={props.item.label}
      disabled={!props.session.read().ready}
      onPointerEnter={glow}
      onPointerLeave={() => props.session.feedback([])}
      onFocus={glow}
      onBlur={() => props.session.feedback([])}
      onPointerDown={() => {
        setHeld(true);
        glow();
      }}
      onPointerUp={() => setHeld(false)}
      onPointerCancel={() => {
        setHeld(false);
        props.session.feedback([]);
      }}
      onClick={() => {
        if (props.session.ready) void props.session.combo(props.item).catch(props.session.error);
      }}
    >
      <For each={props.item.keys}>
        {(key) => (
          <span class={['mini', { rec: key.record }]}>
            <Show
              when={key.record}
              fallback={key.icon ? <Icon name={key.icon} size={12} /> : (key.face ?? '')}
            >
              <i />
            </Show>
          </span>
        )}
      </For>
      <span class="plus">+ POWER</span>
      <span class="what">{props.item.label}</span>
    </button>
  );
}
export function Controls(props: { session?: ControlSession }) {
  return (
    <Show when={props.session}>
      {(session) => (
        <Show when={session().read().data}>
          {(data) => (
            <>
              <For each={data().layout}>{(node) => <Node session={session()} node={node} />}</For>
              <hr />
              <Show when={data().combos.length}>
                <div class="cap">POWER-ON COMBOS</div>
                <div class="combos">
                  <For each={data().combos}>
                    {(item) => <Combo session={session()} item={item} />}
                  </For>
                </div>
              </Show>
            </>
          )}
        </Show>
      )}
    </Show>
  );
}
