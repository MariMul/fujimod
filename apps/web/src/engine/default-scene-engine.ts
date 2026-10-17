import type { BootMedia, EngineConfig, Scene } from './api';
import { WorkerEngine } from './worker-engine.ts';

/** Decode a browser image for the ordinary RGB input path. */
export async function loadImageScene(url: URL): Promise<Scene> {
  const response = await fetch(url);
  if (!response.ok) throw Error('Default scene image is unavailable.');
  const image = await createImageBitmap(await response.blob());
  try {
    const { width, height } = image;
    const context = new OffscreenCanvas(width, height).getContext('2d');
    if (!context) throw Error('Default scene image could not be decoded.');
    context.drawImage(image, 0, 0);
    const rgba = context.getImageData(0, 0, width, height).data;
    const rgb = new Uint8Array(width * height * 3);
    for (let source = 0, target = 0; source < rgba.length; source += 4) {
      rgb[target++] = rgba[source];
      rgb[target++] = rgba[source + 1];
      rgb[target++] = rgba[source + 2];
    }
    return { width, height, rgb };
  } finally {
    image.close();
  }
}

/** App input policy: no supplied source selects the bundled still image. */
export class DefaultSceneEngine extends WorkerEngine {
  private loadDefault: () => Promise<Scene>;
  private defaultScene?: Promise<Scene>;
  private selectedScene: Scene | null = null;
  private bootStarted = false;

  constructor(createWorker: () => Worker, onError: (error: Error) => void,
              loadDefault: () => Promise<Scene>) {
    super(createWorker, onError);
    this.loadDefault = loadDefault;
  }

  override async boot(dat: ArrayBuffer, config: EngineConfig,
                      progress: (message: string, percent: number) => void, media?: BootMedia) {
    this.prepare(config);
    this.defaultScene ??= this.loadDefault();
    const loaded = await this.defaultScene;
    const image = this.selectedScene ?? loaded;
    this.bootStarted = true;
    const inputs = await super.boot(dat, config, progress, media, image);
    await this.setScene(this.selectedScene);
    return inputs;
  }

  override async setScene(scene: Scene | null) {
    this.selectedScene = scene;
    if (!this.bootStarted) return;
    const image = scene ?? await (this.defaultScene ??= this.loadDefault());
    if (this.selectedScene !== scene) return;
    await super.setScene(image);
  }
}
