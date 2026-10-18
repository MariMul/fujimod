export interface CameraPerformance {
  guestTicks: number;
  virtualSeconds: number;
  wallSeconds: number;
  speed: number;
  runtime?: Record<string, unknown>;
}
/** Host observations only. This never changes the CPU ratio or guest clock. */
export class CameraPace {
  private started = 0;
  private ticks = 0;
  reset(ticks: number) {
    this.started = performance.now();
    this.ticks = ticks;
  }
  sample(ticks: number, runtime?: Record<string, unknown>): CameraPerformance {
    const wallSeconds = (performance.now() - this.started) / 1000;
    const virtualSeconds = (ticks - this.ticks) / 12_000_000;
    return {
      guestTicks: ticks,
      virtualSeconds,
      wallSeconds,
      speed: wallSeconds > 0 ? virtualSeconds / wallSeconds : 0,
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
