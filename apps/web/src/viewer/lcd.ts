import { createStore } from 'solid-js';
import { badgeStyle } from '../feedback/input';
import type { Frame } from '../engine/api';

/** Compose guest LCD frames and fault feedback on a 2D canvas. */
export class LCDViewer {
  readonly ui;
  private setUi;
  private lcdImage?: ImageData;
  private faulted = false;
  private faultStart?: number;
  private faultImage?: ImageData;
  private faultAnimation = 0;
  private feedbackTimer?: ReturnType<typeof setTimeout>;

  constructor(
    private host: HTMLElement,
    private lcd: HTMLCanvasElement,
  ) {
    const [ui, setUi] = createStore({ badge: undefined as string | undefined });
    this.ui = ui;
    this.setUi = setUi;
    this.host.dataset.renderer = 'lcd';
  }
  captureView(): string {
    return this.lcd.toDataURL('image/png');
  }
  pressFeedback(id: string) {
    clearTimeout(this.feedbackTimer);
    this.setUi((s) => {
      s.badge = id;
    });
    this.feedbackTimer = setTimeout(() => {
      this.setUi((s) => {
        s.badge = undefined;
      });
    }, badgeStyle.press_ms);
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

    if (this.faulted) this.paintFault();
    this.host.dataset.frames = String(frame.sequence);
  }
  fault(active: boolean) {
    if (active === this.faulted) return;
    cancelAnimationFrame(this.faultAnimation);
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
  }
  private paintFault(now = performance.now()) {
    cancelAnimationFrame(this.faultAnimation);
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
    else this.faultAnimation = requestAnimationFrame(() => this.paintFault());
  }
  dispose() {
    clearTimeout(this.feedbackTimer);
    cancelAnimationFrame(this.faultAnimation);
  }
}
