// Passive, optional observation of owner-side requests. Observers cannot change
// the request, completion, USB ownership or whether a reply reaches the peer.
function notify(callback, event) {
  if (!callback) return;
  try {
    callback(Object.freeze(event));
  } catch {
    // Diagnostics must never change transport behavior.
  }
}
// Diagnostic observers receive private bounded prefixes, never live payload buffers.
const CAPTURE = 4096;
const snapshot = (data) => (data instanceof Uint8Array ? data.slice(0, CAPTURE) : new Uint8Array());
export async function observeRequest(controller, request, execute, callback) {
  const operation = {
    ...request.operation,
    data: callback ? snapshot(request.operation.data) : undefined,
    dataLength: request.operation.data?.byteLength ?? 0,
  };
  if (operation.setup) operation.setup = Object.freeze({ ...operation.setup });
  const identity = {
    controller,
    id: request.id,
    device: request.device,
    operation: Object.freeze(operation),
    timeoutMs: request.timeout_ms,
  };
  const started = performance.now();
  notify(callback, { ...identity, phase: 'submitted' });
  const completion = { devices: [], data: new Uint8Array(), transferred: 0, error: null };
  try {
    Object.assign(completion, await execute());
  } catch (error) {
    completion.error = error?.message || String(error);
  }
  notify(callback, {
    ...identity,
    phase: 'completed',
    elapsedMs: performance.now() - started,
    completion: Object.freeze({
      data: callback ? snapshot(completion.data) : undefined,
      dataLength: completion.data.byteLength,
      transferred: completion.transferred,
      error: completion.error,
      deviceCount: completion.devices?.length ?? 0,
    }),
  });
  return completion;
}
