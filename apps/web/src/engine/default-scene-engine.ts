import type { BootMedia, EngineConfig, Scene } from './api';
import { WorkerEngine } from './worker-engine.ts';

/** App input policy: no supplied source selects the bundled still image. */
export class DefaultSceneEngine extends WorkerEngine {
  private loadDefault: () => Promise<Scene>;
  private defaultScene?: Promise<Scene>;
  private selectedScene: Scene | null = null;
  private bootStarted = false;

  constructor(
    createWorker: () => Worker,
    onError: (error: Error) => void,
    loadDefault: () => Promise<Scene>,
  ) {
    super(createWorker, onError);
    this.loadDefault = loadDefault;
  }

  override async boot(
    dat: ArrayBuffer,
    config: EngineConfig,
    progress: (message: string, percent?: number) => void,
    media?: BootMedia,
  ) {
    progress('Loading camera runtime…');
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
    const image = scene ?? (await (this.defaultScene ??= this.loadDefault()));
    if (this.selectedScene !== scene) return;
    await super.setScene(image);
  }
}
