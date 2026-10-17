import type { Engine, Input } from "../engine/api";
import type { ModelHit, ModelViewer } from "../viewer/model";
import type { ControlNode, DevicePanel, PowerCombo } from "./layout";
import { icon } from "./icons";
import type { DetentSound } from "../feedback/detent-sound";
import { DetentFeedback } from "../feedback/detent";

/** Body data chooses the controls; shared components own gesture laws. */
export class InputPanel {
  private detent = new DetentFeedback();
  private detentSounds: Record<string, DetentSound> = {};
  private inputs = new Map<string, Input>();
  private relativeSteps = new Map<string, number>();
  private modelFractions = new Map<string, number>();
  private positions = new Map<string, string>();
  private controls = new Map<string, HTMLElement[]>();
  private renders = new Map<string, () => void>();
  private holds = new Map<number | string, string>();
  private engine?: Engine;
  private ready = false;
  private powered = true;
  private powerID?: string;
  private stopShutter?: (release?: boolean) => void;
  private stopRotation?: () => void;
  private rotation?: { dial: HTMLElement; fraction: number; center?: number };
  private pulseQueue: Promise<unknown> = Promise.resolve();
  private observers: ResizeObserver[] = [];
  constructor(
    private host: HTMLElement,
    private viewer: ModelViewer,
    private error: (error: unknown) => void,
    private combo: (combo: PowerCombo) => Promise<void>,
  ) {
    viewer.onRotaryPreview = (id, fraction) => {
      fraction = this.clampFraction(id, fraction);
      this.modelFractions.set(id, fraction);
      const ticks = this.host.querySelector<HTMLElement>(`.dial.ticks[data-id="${CSS.escape(id)}"] .vals`);
      if (ticks) ticks.style.backgroundPositionX = `${((this.relativeSteps.get(id) ?? 0) + fraction) * 16}px`;
      this.renders.get(id)?.();
      return fraction;
    };
    viewer.onPointerDown = (hit, event) => this.modelDown(hit, event);
    viewer.onHover = (id) => this.feedback(id ? [id] : []);
    window.addEventListener("pointerup", (event) =>
      this.release(event.pointerId),
    );
    window.addEventListener("pointercancel", (event) =>
      this.release(event.pointerId),
    );
    window.addEventListener("blur", () => this.releaseAll());
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.releaseAll();
    });
  }
  render(
    inputs: Input[],
    engine: Engine,
    data: DevicePanel,
    initial: Record<string, string> = {},
  ) {
    this.clear();
    this.engine = engine;
    this.detentSounds = data.detentSounds ?? {};
    this.ready = true;
    this.inputs = new Map(inputs.map((input) => [input.id, {
      ...input, label: data.inputLabels?.[input.id] ?? input.label,
    }]));
    for (const input of inputs)
      if (input.positions?.length)
        this.positions.set(
          input.id,
          initial[input.id] ?? input.initial ?? input.positions[0].id,
        );
    for (const node of data.layout) this.host.append(this.node(node));
    this.host.append(document.createElement("hr"));
    const caption = this.caption("POWER-ON COMBOS"),
      combos = document.createElement("div");
    combos.className = "combos";
    for (const item of data.combos) {
      const row = document.createElement("button");
      row.className = "combo";
      row.type = "button";
      row.setAttribute("aria-label", item.label);
      for (const key of item.keys) {
        const mini = document.createElement("span");
        mini.className = "mini" + (key.record ? " rec" : "");
        if (key.record) mini.append(document.createElement("i"));
        else if (key.icon) mini.append(icon(key.icon, 12));
        else mini.textContent = key.face ?? "";
        row.append(mini);
      }
      const plus = document.createElement("span");
      plus.className = "plus";
      plus.textContent = "+ POWER";
      const what = document.createElement("span");
      what.className = "what";
      what.textContent = item.label;
      row.append(plus, what);
      const glow = () =>
        this.feedback(
          item.inputs.map((input) => input.id),
          true,
        );
      row.onpointerenter = glow;
      row.onpointerleave = () => this.feedback([]);
      row.onfocus = glow;
      row.onblur = () => this.feedback([]);
      row.onpointerdown = () => {
        row.classList.add("held");
        glow();
      };
      row.onpointerup = () => row.classList.remove("held");
      row.onpointercancel = () => {
        row.classList.remove("held");
        this.feedback([]);
      };
      row.onclick = () => {
        if (this.ready) void this.combo(item).catch(this.error);
      };
      combos.append(row);
    }
    if (data.combos.length) this.host.append(caption, combos);
    this.viewer.configureRotary(
      inputs, (id, direction) => this.step(id, direction), this.detentSounds,
    );
    for (const id of this.positions.keys()) this.syncRotary(id);
    this.viewer.setRotaryEnabled(true);
  }
  private node(node: ControlNode): HTMLElement {
    if (node.kind === "divider") return document.createElement("hr");
    if (node.kind === "space") {
      const space = document.createElement("div");
      space.className = "sp";
      return space;
    }
    if (node.kind === "command") {
      const box = document.createElement("div");
      box.className = "cmd";
      box.append(this.caption(node.caption!));
      const tray = document.createElement("div");
      tray.className = "knurl";
      for (const [key, face, cls] of [
        ["left", "◀", "side l"],
        ["center", "", "tab"],
        ["right", "▶", "side r"],
      ]) {
        const button = this.key(node.bindings![key], { face: [face] });
        button.className = cls;
        tray.append(button);
      }
      box.append(tray);
      return box;
    }
    if (node.kind === "dpad") {
      const pad = document.createElement("div");
      pad.className = "dpad";
      for (const [key, face, cls] of [
        ["up", "▲", "a u"],
        ["down", "▼", "a d"],
        ["left", "◀", "a l"],
        ["right", "▶", "a r"],
        ["center", node.face?.join("\n") ?? "", "ok"],
      ]) {
        const button = this.key(node.bindings![key], {
          face: face.split("\n"),
        });
        button.className = cls;
        pad.append(button);
      }
      return pad;
    }
    if (node.kind !== "control") {
      const group = document.createElement("div");
      group.className = node.className ?? "";
      for (const child of node.children ?? []) group.append(this.node(child));
      return group;
    }
    const id = node.id!,
      input = this.inputs.get(id);
    if (node.operation === "power") this.powerID = id;
    if (node.variant === "key" || node.variant === "record") {
      const button = this.key(id, node);
      button.classList.add(...(node.className?.split(" ") ?? []));
      if (!node.caption) return button;
      const box = document.createElement("div");
      box.className = `control-node ${node.className ?? ""}`;
      box.append(this.caption(node.caption), button);
      return box;
    }
    const box = document.createElement("div");
    box.className = `control-node ${node.className ?? ""}`;
    box.dataset.id = id;
    if (node.caption) box.append(this.caption(node.caption));
    if (node.variant === "segment") {
      const positions =
        input?.positions ??
        Object.keys(node.labels ?? {}).map((id) => ({ id, label: id }));
      if (!this.positions.has(id))
        this.positions.set(id, positions[0]?.id ?? "");
      const seg = document.createElement("div");
      seg.className = "seg wide";
      seg.setAttribute("role", "group");
      seg.setAttribute("aria-label", input?.label ?? node.caption ?? id);
      const update = () => {
        for (const option of seg.querySelectorAll<HTMLButtonElement>(
          "button",
        )) {
          const selected = option.dataset.value === this.positions.get(id);
          option.classList.toggle("on", selected);
          option.setAttribute("aria-pressed", String(selected));
        }
      };
      for (const position of positions) {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.value = position.id;
        button.textContent = node.labels?.[position.id] ?? position.label;
        button.setAttribute(
          "aria-label",
          `${input?.label ?? node.caption} ${button.textContent}`,
        );
        button.onclick = () => this.position(id, position.id);
        seg.append(button);
        this.track(id, button);
      }
      this.renders.set(id, update);
      update();
      box.append(seg);
    } else if (node.variant === "shutter") box.append(this.shutter(id));
    else {
      const dial = document.createElement("div");
      dial.className = "dial" + (node.variant === "ticks" ? " ticks" : "");
      dial.tabIndex = 0;
      dial.setAttribute("role", "slider");
      dial.setAttribute("aria-label", input?.label ?? node.caption ?? id);
      dial.dataset.id = id;
      const values = document.createElement("div");
      values.className = "vals";
      dial.append(values);
      const positions = this.inputs.get(id)?.positions ?? [];
      const labels: HTMLElement[] = [];
      for (const position of positions) {
        const value = document.createElement("span");
        value.dataset.value = position.id;
        value.textContent = node.labels?.[position.id] ?? position.label;
        values.append(value);
        labels.push(value);
      }
      let pending = false;
      const selectedIndex = () => Math.max(
        0, positions.findIndex((p) => p.id === this.positions.get(id)),
      );
      const update = () => {
        if (node.variant === "ticks") return;
        for (const value of labels)
          value.classList.toggle("on", value.dataset.value === this.positions.get(id));
        dial.setAttribute("aria-valuenow", String(selectedIndex()));
        dial.setAttribute("aria-valuemin", "0");
        dial.setAttribute(
          "aria-valuemax", String(Math.max(0, positions.length - 1)),
        );
        dial.setAttribute("aria-valuetext", this.positions.get(id) ?? "");
        if (pending) return;
        pending = true;
        requestAnimationFrame(() => {
          pending = false;
          if (!dial.isConnected) return;
          // Reserve each label's selected width so changing the selected face
          // cannot reflow the strip underneath a captured pointer.
          for (const value of labels) {
            if (!value.style.width) {
              const selected = value.classList.contains("on");
              value.classList.add("on");
              value.style.width = `${value.offsetWidth}px`;
              value.style.flexShrink = "0";
              value.classList.toggle("on", selected);
            }
          }
          if (this.rotation?.dial === dial && this.rotation.center !== undefined) {
            values.style.transform = `translateX(${dial.clientWidth / 2 - this.rotation.center}px)`;
            return;
          }
          const index = selectedIndex();
          const fraction = this.rotation?.dial === dial ? this.rotation.fraction : (this.modelFractions.get(id) ?? 0);
          const visual = Math.max(0, Math.min(positions.length - 1, index + fraction));
          const left = labels[Math.floor(visual)];
          const right = labels[Math.ceil(visual)];
          if (left && right) {
            const center = (item: HTMLElement) => item.offsetLeft + item.offsetWidth / 2;
            const offset = center(left) + (center(right) - center(left)) * (visual % 1);
            values.style.transform = `translateX(${dial.clientWidth / 2 - offset}px)`;
          }
        });
      };
      this.renders.set(id, update);
      update();
      const observer = new ResizeObserver(update);
      observer.observe(dial);
      this.observers.push(observer);
      dial.onpointerdown = (event) => this.rotate(id, dial, event);
      dial.onkeydown = (event) => {
        if (
          ["ArrowLeft", "ArrowDown", "ArrowRight", "ArrowUp"].includes(
            event.key,
          )
        ) {
          event.preventDefault();
          this.detent.beginGesture(event);
          if (this.step(
            id,
            ["ArrowLeft", "ArrowDown"].includes(event.key) ? -1 : 1,
          )) this.detent.step(this.detentSounds[id]);
        }
      };
      this.track(id, dial);
      box.append(dial);
    }
    return box;
  }
  private caption(text: string) {
    const label = document.createElement("div");
    label.className = "cap";
    label.textContent = text;
    return label;
  }
  private track(id: string, node: HTMLElement) {
    const nodes = this.controls.get(id) ?? [];
    nodes.push(node);
    this.controls.set(id, nodes);
    const input = this.inputs.get(id);
    if (node instanceof HTMLButtonElement) node.disabled = !input?.supported;
    else node.setAttribute("aria-disabled", String(!input?.supported));
  }
  private key(
    id: string,
    node: Pick<ControlNode, "face" | "icon" | "hint" | "variant">,
  ) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "key" + (node.variant === "record" ? " rec" : "");
    button.dataset.id = id;
    button.setAttribute("aria-label", this.inputs.get(id)?.label ?? id);
    if (node.variant === "record") button.append(document.createElement("i"));
    else if (node.icon) button.append(icon(node.icon));
    else {
      const text = document.createElement("span");
      if ((node.face?.length ?? 0) > 1) text.className = "two";
      text.textContent =
        node.face?.join("\n") ?? this.inputs.get(id)?.label ?? id;
      button.append(text);
    }
    if (node.hint) {
      button.classList.add("wifi");
      const hint = document.createElement("em");
      hint.textContent = node.hint;
      button.append(hint);
    }
    button.onpointerdown = (event) => {
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      this.press(id, event.pointerId);
    };
    button.onclick = (event) => {
      if (event.detail === 0 && this.ready) void this.pulse(id);
    };
    button.onpointerenter = () => this.feedback([id]);
    button.onpointerleave = () => {
      if (!this.holds.size) this.feedback([]);
    };
    this.track(id, button);
    return button;
  }
  private shutter(id: string) {
    const tray = document.createElement("div");
    tray.className = "slide idle";
    tray.tabIndex = 0;
    tray.dataset.id = id;
    tray.setAttribute("role", "button");
    tray.setAttribute("aria-label", this.inputs.get(id)?.label ?? id);
    const thumb = document.createElement("span");
    thumb.className = "thumb";
    const end = document.createElement("span");
    end.className = "end";
    end.textContent = "FULL";
    tray.append(thumb, end);
    this.track(id, tray);
    tray.onpointerdown = (event) => {
      if (!this.ready || !this.powered) return;
      event.preventDefault();
      tray.setPointerCapture(event.pointerId);
      const started = performance.now(),
        bounds = tray.getBoundingClientRect(),
        origin = event.clientX,
        engine = this.engine!;
      let full = false,
        moved = false;
      const stage = (value: "half" | "full") => {
        full = value === "full";
        tray.className = `slide ${value}`;
        thumb.textContent = value.toUpperCase();
        void engine.press(id, value).catch((error) => {
          if (this.engine === engine && this.ready) this.error(error);
        });
      };
      stage("half");
      const move = (next: PointerEvent) => {
        if (next.pointerId !== event.pointerId) return;
        const delta = next.clientX - origin;
        moved ||= Math.abs(delta) > 5;
        const travel = Math.max(1, bounds.width - 70);
        if (delta >= travel && !full) stage("full");
        else if (delta < travel * 0.8 && full) stage("half");
        thumb.style.left = full
          ? ""
          : `${3 + Math.max(0, Math.min(travel, delta))}px`;
      };
      const cleanup = (release = false) => {
        if (release) {
          const engine = this.engine;
          void engine?.release(id).catch((error) => {
            if (this.engine === engine && this.ready) this.error(error);
          });
        }
        tray.className = "slide idle";
        thumb.textContent = "";
        thumb.style.left = "";
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", finish);
        window.removeEventListener("pointercancel", finish);
        this.stopShutter = undefined;
      };
      const finish = (up: PointerEvent) => {
        if (up.pointerId !== event.pointerId) return;
        if (
          up.type !== "pointercancel" &&
          !moved &&
          !full &&
          performance.now() - started < 250
        )
          stage("full");
        const engine = this.engine!;
        void engine.release(id).catch((error) => {
          if (this.engine === engine && this.ready) this.error(error);
        });
        cleanup();
      };
      this.stopShutter = cleanup;
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", finish);
      window.addEventListener("pointercancel", finish);
    };
    tray.onkeydown = (event) => {
      if ((event.key === " " || event.key === "Enter") && !event.repeat) {
        event.preventDefault();
        if (this.ready && this.powered) {
          const engine = this.engine!;
          void engine
            .press(id, "half")
            .then(() => engine.press(id, "full"))
            .then(() => engine.release(id))
            .catch((error) => {
              if (this.engine === engine && this.ready) this.error(error);
            });
        }
      }
    };
    return tray;
  }
  private clampFraction(id: string, fraction: number) {
    const positions = this.inputs.get(id)?.positions;
    if (!positions?.length) return fraction;
    const index = positions.findIndex(p => p.id === this.positions.get(id));
    return Math.max(-index, Math.min(positions.length - 1 - index, fraction));
  }
  private syncRotary(id: string) {
    const input = this.inputs.get(id);
    if (!input?.positions?.length) return;
    const index = input.positions.findIndex(p => p.id === this.positions.get(id));
    const initial = input.positions.findIndex(p => p.id === (input.initial ?? input.positions![0].id));
    if (index >= 0) this.viewer.setRotaryPosition(id, index - Math.max(0, initial));
  }
  private position(id: string, value: string) {
    if (
      !this.ready ||
      (!this.powered && id !== this.powerID) ||
      !this.inputs.get(id)?.supported ||
      this.positions.get(id) === value
    )
      return;
    this.positions.set(id, value);
    this.syncRotary(id);
    this.renders.get(id)?.();
    this.feedback([id]);
    const engine = this.engine!;
    if (id === this.powerID) {
      this.powered = value === "on";
      if (this.powered) this.ready = false;
      this.availability();
    }
    void engine
      .setPosition(id, value)
      .then(() => {
        if (this.engine === engine && id === this.powerID && value === "on") {
          this.ready = true;
          this.availability();
        }
      })
      .catch((error) => {
        if (this.engine === engine) this.error(error);
      });
  }
  private step(id: string, direction: number): boolean {
    const input = this.inputs.get(id);
    if (!this.ready || !this.powered || !input?.supported) return false;
    if (input.relative) {
      this.relativeSteps.set(id, (this.relativeSteps.get(id) ?? 0) + Math.sign(direction));
      const ticks = this.host.querySelector<HTMLElement>(`.dial.ticks[data-id="${CSS.escape(id)}"] .vals`);
      if (ticks) ticks.style.backgroundPositionX = `${this.relativeSteps.get(id)! * 16}px`;
      this.viewer.turnRotary(id, Math.sign(direction));
      void this.engine!.setPosition(id, Math.sign(direction)).catch(this.error);
      return true;
    }
    if (input.positions?.length) {
      const current = input.positions.findIndex(
        (p) => p.id === this.positions.get(id),
      );
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
  private pulse(id: string) {
    const engine = this.engine!,
      task = this.pulseQueue
        .then(() => { this.viewer.pressFeedback(id); return engine.press(id); })
        .then(() => engine.release(id));
    this.pulseQueue = task.catch((error) => {
      if (this.engine === engine && this.ready) this.error(error);
    });
    return task;
  }
  private press(
    id: string,
    pointer: number | string,
    stage?: string,
    point?: { x: number; y: number },
    surfacePoint?: ModelHit["surfacePoint"],
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
    this.viewer.pressFeedback(id, this.inputs.get(id)?.kind === "touch" ? surfacePoint : undefined);
    void this.engine!.press(id, stage, point).catch(this.error);
  }
  private release(pointer: number | string) {
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
  private releaseAll() {
    this.stopRotation?.();
    this.stopShutter?.(true);
    for (const key of [...this.holds.keys()]) this.release(key);
  }
  private feedback(ids: string[], glow = false) {
    for (const [id, nodes] of this.controls)
      for (const node of nodes) {
        node.classList.toggle("active", ids.includes(id) && !glow);
        node.classList.toggle("glow", ids.includes(id) && glow);
      }
    this.viewer.highlight(ids);
  }
  private rotate(id: string, dial: HTMLElement, event: PointerEvent) {
    if (
      event.button !== 0 || !this.ready || !this.powered ||
      !this.inputs.get(id)?.supported
    ) return;
    if (this.stopRotation) return;
    this.detent.beginGesture(event);
    event.preventDefault();
    dial.setPointerCapture(event.pointerId);
    this.viewer.setOrbit(false);
    const values = dial.querySelector<HTMLElement>(".vals")!;
    const ticks = dial.classList.contains("ticks");
    const initialTicks = parseFloat(values.style.backgroundPositionX) || 0;
    const centers = Array.from(values.children, value => {
      const label = value as HTMLElement;
      return label.offsetLeft + label.offsetWidth / 2;
    });
    const rotation: NonNullable<InputPanel["rotation"]> = { dial, fraction: 0 };
    if (centers.length) {
      // Start from the rendered position, even when a previous snap is in flight.
      const transform = getComputedStyle(values).transform;
      const x = new DOMMatrixReadOnly(transform).m41;
      rotation.center = dial.clientWidth / 2 - x;
      values.style.transform = `translateX(${x}px)`;
    }
    this.rotation = rotation;
    values.style.transition = "none";
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
        const nearest = centers.reduce((best, candidate, index) =>
          Math.abs(candidate - center) < Math.abs(centers[best] - center) ? index : best, 0);
        const current = Number(dial.getAttribute("aria-valuenow"));
        const direction = Math.sign(nearest - current);
        for (let n = 0; n < Math.abs(nearest - current); n++) {
          if (this.step(id, direction)) this.detent.step(this.detentSounds[id]);
        }
        const index = Number(dial.getAttribute("aria-valuenow"));
        const distance = center - centers[index];
        const neighbor = centers[index + Math.sign(distance)];
        rotation.fraction = neighbor === undefined || distance === 0
          ? 0 : distance / Math.abs(neighbor - centers[index]);
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
      this.viewer.previewRotary(id, rotation.fraction, true);
      if (ticks) values.style.backgroundPositionX = `${tickPosition + remainder}px`;
      else this.renders.get(id)?.();
    };
    const end = (up: PointerEvent) => {
      if (up.pointerId === event.pointerId) cleanup();
    };
    const cleanup = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      dial.removeEventListener("lostpointercapture", end);
      this.rotation = undefined;
      this.viewer.previewRotary(id, 0, false);
      values.style.transition = ticks
        ? "background-position-x 140ms ease-out"
        : "transform 140ms ease-out";
      if (ticks) values.style.backgroundPositionX = `${tickPosition}px`;
      else this.renders.get(id)?.();
      if (dial.hasPointerCapture(event.pointerId)) dial.releasePointerCapture(event.pointerId);
      this.viewer.setOrbit(true);
      this.stopRotation = undefined;
    };
    this.stopRotation = cleanup;
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    dial.addEventListener("lostpointercapture", end);
  }
  private modelDown(hit: ModelHit, event: PointerEvent) {
    const input = this.inputs.get(hit.id);
    if (
      !input?.supported ||
      !this.ready ||
      input.kind === "dial" ||
      input.kind === "ring"
    )
      return;
    event.preventDefault();
    event.stopPropagation();
    this.viewer.setOrbit(false);
    this.viewer.renderer.domElement.setPointerCapture(event.pointerId);
    if (input.positions?.length) this.step(hit.id, 1);
    else
      this.press(
        hit.id,
        event.pointerId,
        input.stages ? "full" : undefined,
        hit.point,
        hit.surfacePoint,
      );
  }
  private availability() {
    for (const [id, nodes] of this.controls)
      for (const node of nodes) {
        const disabled =
          !this.ready ||
          !this.inputs.get(id)?.supported ||
          (!this.powered && id !== this.powerID);
        if (node instanceof HTMLButtonElement) node.disabled = disabled;
        node.setAttribute("aria-disabled", String(disabled));
      }
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
      this.positions.set(this.powerID, "on");
      this.renders.get(this.powerID)?.();
    }
    this.availability();
  }
  synchronize(positions: Record<string, string>) {
    for (const [id, value] of Object.entries(positions)) {
      this.positions.set(id, value);
      this.syncRotary(id);
      this.renders.get(id)?.();
    }
    if (this.powerID && positions[this.powerID])
      this.powered = positions[this.powerID] === "on";
    this.availability();
  }
  clear() {
    this.releaseAll();
    this.ready = false;
    this.stopShutter?.();
    this.powered = true;
    this.powerID = undefined;
    this.viewer.setRotaryEnabled(false);
    for (const observer of this.observers) observer.disconnect();
    this.observers = [];
    this.host.replaceChildren();
    this.inputs.clear();
    this.detentSounds = {};
    this.positions.clear();
    this.modelFractions.clear();
    this.relativeSteps.clear();
    this.controls.clear();
    this.renders.clear();
    this.engine = undefined;
  }
}
