type RendererModule = {
  default(input?: URL): Promise<unknown>;
  render_capture(capture: Uint8Array): Promise<unknown>;
};

export type WorkerReply = { ok: true; proof: unknown } | { ok: false; error: string };

const worker = self as unknown as DedicatedWorkerGlobalScope;

worker.addEventListener('message', async () => {
  try {
    const moduleUrl = new URL('/dmp_webgpu.js', worker.location.href),
      wasmUrl = new URL('/dmp_webgpu_bg.wasm', worker.location.href);
    const loaded: unknown = await import(moduleUrl.href);
    const renderer = loaded as RendererModule;
    await renderer.default(wasmUrl);
    const response = await fetch('/capture.dmpcap');
    if (!response.ok) throw Error('Saved DMP capture is unavailable.');
    const capture = new Uint8Array(await response.arrayBuffer());
    const proof = await renderer.render_capture(capture);
    worker.postMessage({ ok: true, proof } satisfies WorkerReply);
  } catch (error) {
    worker.postMessage({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    } satisfies WorkerReply);
  }
});
