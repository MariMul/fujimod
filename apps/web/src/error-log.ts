import type { EngineFault } from '@fujimod/emulator-ui/engine/fault';
declare const __FUJIMOD_SIMULATOR_BUILD__: string;

export const ERROR_LOG_URL = 'https://share.fujimod.com/error-logs';
const SITES = new Set(['fujimod.com', 'www.fujimod.com']);
const LIMIT = 20;
const TEXT = 4000;

export type ErrorLogKind = 'error' | 'rejection' | 'console' | 'fault';
export interface ErrorLogRecord {
  id: string;
  /** One per page load; groups every record from the same open page. */
  session: string;
  kind: ErrorLogKind;
  message: string;
  stack?: string;
  fault?: EngineFault;
  /** Origin and path only: queries and fragments can carry share links. */
  page: string;
  build: string;
  userAgent: string;
  createdAt: string;
  /** Milliseconds since the page started loading. */
  uptime: number;
}

const clip = (text: string) => (text.length > TEXT ? `${text.slice(0, TEXT)}…` : text);
function describe(value: unknown): { message: string; stack?: string } {
  if (value instanceof Error)
    return { message: clip(`${value.name}: ${value.message}`), stack: value.stack && clip(value.stack) };
  if (typeof value === 'string') return { message: clip(value) };
  try {
    return { message: clip(JSON.stringify(value) ?? String(value)) };
  } catch {
    return { message: clip(String(value)) };
  }
}

/** Sends uncaught errors, console errors and emulator faults from the public site to
 * the error-log intake. Installed before the app hydrates so hydration failures are kept. */
export function installErrorLog(send: (record: ErrorLogRecord) => void = post) {
  const seen = new Set<string>();
  const session = crypto.randomUUID();
  let sent = 0;
  function log(kind: ErrorLogKind, value: unknown, fault?: EngineFault) {
    const { message, stack } = describe(value);
    const key = `${kind}\n${message}`;
    if (sent >= LIMIT || seen.has(key)) return;
    seen.add(key);
    sent++;
    // The intake refuses records over 16 KiB; oversized fault details become text.
    const details = fault && JSON.stringify(fault.details);
    if (fault && details && details.length > TEXT) fault = { ...fault, details: { text: clip(details) } };
    send({
      id: crypto.randomUUID(),
      session,
      kind,
      message,
      stack,
      fault,
      page: location.origin + location.pathname,
      build: __FUJIMOD_SIMULATOR_BUILD__,
      userAgent: navigator.userAgent,
      createdAt: new Date().toISOString(),
      uptime: Math.round(performance.now()),
    });
  }
  addEventListener('error', (event) => log('error', event.error ?? event.message));
  addEventListener('unhandledrejection', (event) => log('rejection', event.reason));
  addEventListener('fujimod-fault', (event) => {
    const fault = (event as CustomEvent<EngineFault>).detail;
    const cause = [fault.details.type, fault.details.error].filter(Boolean).join(': ');
    log('fault', cause ? `${fault.message} ${cause}` : fault.message, fault);
  });
  const error = console.error.bind(console);
  console.error = (...values: unknown[]) => {
    error(...values);
    const first = values.find((value) => value instanceof Error);
    log('console', first ?? values.map((value) => describe(value).message).join(' '));
  };
}

function post(record: ErrorLogRecord) {
  // text/plain keeps this a simple CORS request; keepalive survives page unloads.
  void fetch(ERROR_LOG_URL, {
    method: 'POST',
    mode: 'cors',
    keepalive: true,
    headers: { 'Content-Type': 'text/plain' },
    body: JSON.stringify(record),
  }).catch(() => {});
}

if (typeof window !== 'undefined' && SITES.has(location.hostname)) installErrorLog();
