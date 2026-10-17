// Injected before app modules by the development server; absent from release builds.
(() => {
  const session = crypto.randomUUID();
  let reports = 0;
  const clean = (value) =>
    String(value)
      .replace(/(https?:\/\/[^\s?#]+)[?#][^\s]*/g, '$1')
      .slice(0, 6000);
  const describe = (value) =>
    value instanceof Error
      ? clean(value.stack || value.message)
      : clean(typeof value === 'string' ? value : JSON.stringify(value));
  function report(kind, detail) {
    if (++reports > 60) return;
    const viewer = document.querySelector('#viewer');
    const canvas = document.querySelector('#simulation-canvas');
    const state = viewer?.dataset;
    const body = JSON.stringify({
      session,
      kind,
      detail: clean(detail),
      elapsed: Math.round(performance.now()),
      userAgent: navigator.userAgent,
      webgpu: !!navigator.gpu,
      page: location.pathname,
      visibility: document.visibilityState,
      renderer: state?.renderer,
      backend: state?.rendererBackend,
      wasmMemoryBytes: state?.wasmMemoryBytes,
      emulatorMemoryBytes: window.cameraPerformance?.runtime?.wasmMemoryBytes,
      emulatorPeakMemoryBytes: window.cameraPerformance?.runtime?.peakWasmMemoryBytes,
      rendererError: state?.rendererError,
      stage: state?.rendererStage,
      simulation: state?.simulation,
      polls: state?.renderFrames,
      canvas: canvas && {
        width: canvas.width,
        height: canvas.height,
        cssWidth: canvas.clientWidth,
        cssHeight: canvas.clientHeight,
      },
      resources: performance
        .getEntriesByType('resource')
        .filter((entry) => /\.(wasm|glb)(\?|$)/.test(entry.name))
        .map((entry) => ({
          path: new URL(entry.name).pathname,
          duration: Math.round(entry.duration),
          bytes: entry.decodedBodySize,
          transferred: entry.transferSize,
          status: entry.responseStatus,
        })),
    });
    void fetch('/__browser-diagnostics', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      keepalive: true,
    }).catch(() => {});
  }
  for (const level of ['warn', 'error']) {
    const original = console[level];
    console[level] = function (...args) {
      original.apply(console, args);
      try {
        report(`console.${level}`, args.map(describe).join(' '));
      } catch {
        /* Logging must not break rendering. */
      }
    };
  }
  window.addEventListener(
    'error',
    (event) => {
      report(
        'error',
        event.error
          ? describe(event.error)
          : event.message ||
              `Resource failed: ${event.target?.src || event.target?.href || 'unknown'}`,
      );
    },
    true,
  );
  window.addEventListener('unhandledrejection', (event) =>
    report('unhandledrejection', describe(event.reason)),
  );
  document.addEventListener('visibilitychange', () => report('visibility', ''));
  report('startup', '');
  for (const seconds of [5, 15, 30, 60]) setTimeout(() => report('checkpoint', ''), seconds * 1000);
})();
