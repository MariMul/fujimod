/** Runtime Wasm compilation is allowed; JavaScript eval remains disabled. */
export const contentSecurityPolicy =
  "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; worker-src 'self' blob:; object-src 'none'; base-uri 'self'";

/** Shared memory is available to the emulator owner and its device pthreads. */
export const runtimeHeaders = {
  'Content-Security-Policy': contentSecurityPolicy,
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
} as const;
