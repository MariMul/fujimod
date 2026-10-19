export interface CameraPerformance {
  guestTicks: number;
  virtualSeconds: number;
  wallSeconds: number;
  speed: number;
  averageSpeed: number;
  runtime?: Record<string, unknown>;
}
/** Host observations only. This never changes the CPU ratio or guest clock. */
export class CameraPace {
  private started = 0;
  private ticks = 0;
  private live = false;
  private samples: { ticks: number; wall: number }[] = [];
  reset(ticks: number) {
    this.started = performance.now();
    this.ticks = ticks;
    this.live = false;
    this.samples = [];
  }
  rebase(ticks: number) {
    this.ticks = ticks;
    this.live = true;
    this.samples = [];
  }
  sample(ticks: number, runtime?: Record<string, unknown>): CameraPerformance {
    const now = performance.now();
    const wallSeconds = (now - this.started) / 1000;
    const virtualSeconds = (ticks - this.ticks) / 12_000_000;
    const averageSpeed = wallSeconds > 0 ? virtualSeconds / wallSeconds : 0;
    let speed = averageSpeed;
    if (this.live) {
      if (ticks < (this.samples.at(-1)?.ticks ?? ticks)) this.samples = [];
      this.samples.push({ ticks, wall: now });
      // Keep one observation at the start of the last five seconds. Startup
      // remains in the cumulative totals without diluting current throughput.
      while (this.samples.length > 2 && this.samples[1]!.wall <= now - 5000) this.samples.shift();
      const first = this.samples[0]!;
      speed = now > first.wall ? (ticks - first.ticks) / 12000 / (now - first.wall) : 0;
    }
    return {
      guestTicks: ticks,
      virtualSeconds,
      wallSeconds,
      speed,
      averageSpeed,
      runtime,
    };
  }
}
export class RequestTimings {
  readonly actions: Record<string, { calls: number; seconds: number; maxSeconds: number }> = {};
  measure<T>(action: string, operation: () => T): T {
    const started = performance.now();
    try {
      return operation();
    } finally {
      const seconds = (performance.now() - started) / 1000;
      const timing = (this.actions[action] ??= {
        calls: 0,
        seconds: 0,
        maxSeconds: 0,
      });
      timing.calls++;
      timing.seconds += seconds;
      timing.maxSeconds = Math.max(timing.maxSeconds, seconds);
    }
  }
}
declare global {
  interface Window {
    cameraPerformance?: CameraPerformance;
    profileCamera?: (limit?: number) => Promise<unknown>;
  }
}
