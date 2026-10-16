export interface EngineFault {
  kind: 'unsupported-device' | 'guest-fault' | 'wasm-trap' | 'exception';
  message: string;
  details: Record<string, unknown>;
}
export class EngineError extends Error {
  readonly fault: EngineFault;
  constructor(fault: EngineFault) { super(fault.message); this.fault = fault; this.name = 'EngineError'; }
}
export function faultFrom(value: unknown): EngineFault {
  if (value instanceof EngineError) return value.fault;
  const details: Record<string, unknown> = value && typeof value === 'object'
    ? value instanceof Error ? { type: value.name, error: value.message, stack: value.stack } : { ...value }
    : { error: String(value) };
  const text = [details.type, details.error_type, details.error, details.message, details.exception].filter(Boolean).join(' ');
  const kind = /Unsupported(?:Device|Scanout)|unsupported device/i.test(text) ? 'unsupported-device'
    : value instanceof WebAssembly.RuntimeError || /RuntimeError:.*(?:unreachable|out of bounds)|wasm trap/i.test(text) ? 'wasm-trap'
    : ('cpu' in details || 'pc' in details || /guest|mmio|unmapped|instruction|cpu|abort|fault/i.test(text)) ? 'guest-fault' : 'exception';
  const message = kind === 'unsupported-device' ? 'The camera reached a device operation this emulator does not support.'
    : kind === 'wasm-trap' ? 'The emulator runtime stopped unexpectedly.'
    : kind === 'guest-fault' ? 'The camera stopped after an execution fault.' : 'The emulator stopped unexpectedly.';
  return { kind, message, details };
}
export function stateFault(state: { failure?: unknown; execution_error?: unknown }) {
  return state.execution_error || state.failure ? faultFrom(state.execution_error || state.failure) : undefined;
}
