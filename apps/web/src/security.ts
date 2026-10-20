/** Runtime Wasm compilation is allowed; JavaScript eval remains disabled. */
export const contentSecurityPolicy =
  "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; worker-src 'self' blob:; object-src 'none'; base-uri 'self'";
