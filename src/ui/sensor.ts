import type { Engine } from '../engine/api';
export class SensorInput {
  private stream?: MediaStream;
  private video?: HTMLVideoElement;
  private image?: HTMLImageElement;
  private timer = 0;
  private canvas = document.createElement('canvas');
  private engine?: Engine;
  constructor(private status: (message: string) => void) {}
  connect(engine?: Engine) { this.engine = engine; if (engine && this.image) this.send(); }
  async useImage(file: File) {
    this.stopWebcam();
    const image = new Image(), url = URL.createObjectURL(file);
    try { image.src = url; await image.decode(); this.image = image; this.send(); this.status(file.name); }
    finally { URL.revokeObjectURL(url); }
  }
  async webcam(enabled: boolean) {
    this.stopWebcam();
    if (!enabled) { if (this.image) this.send(); else await this.engine?.setScene(null); this.status(this.image ? 'Image' : 'Built-in scene'); return; }
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'environment' }, audio: false });
      this.video = document.createElement('video'); this.video.muted = true; this.video.playsInline = true; this.video.srcObject = this.stream;
      await this.video.play(); this.status('Webcam');
      const loop = () => { this.send(); this.timer = window.setTimeout(loop, 1000); }; loop();
    } catch (error) { this.stopWebcam(); throw error; }
  }
  private send() {
    const source = this.video?.readyState && this.video.readyState >= 2 ? this.video : this.image;
    if (!source || !this.engine) return;
    const width = source instanceof HTMLVideoElement ? source.videoWidth : source.naturalWidth;
    const height = source instanceof HTMLVideoElement ? source.videoHeight : source.naturalHeight;
    if (!width || !height) return;
    const scale = Math.min(1, 1600 / Math.max(width, height));
    this.canvas.width = Math.max(1, Math.round(width * scale)); this.canvas.height = Math.max(1, Math.round(height * scale));
    const context = this.canvas.getContext('2d', { willReadFrequently: true })!;
    context.drawImage(source, 0, 0, this.canvas.width, this.canvas.height);
    const data = context.getImageData(0, 0, this.canvas.width, this.canvas.height).data;
    const rgb = new Uint8Array(this.canvas.width * this.canvas.height * 3);
    for (let s = 0, t = 0; s < data.length; s += 4, t += 3) { rgb[t] = data[s]; rgb[t + 1] = data[s + 1]; rgb[t + 2] = data[s + 2]; }
    void this.engine.setScene({ rgb, width: this.canvas.width, height: this.canvas.height }).catch(error => this.status(String(error)));
  }
  private stopWebcam() { clearTimeout(this.timer); this.stream?.getTracks().forEach(track => track.stop()); this.stream = undefined; this.video = undefined; }
  dispose() { this.stopWebcam(); this.engine = undefined; }
}
