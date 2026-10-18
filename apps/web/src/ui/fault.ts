import type { EngineFault } from '../engine/fault';
import { icon } from './icons';
export interface FaultContext {
  tick?: number;
  firmware?: string;
  build: string;
}
export function faultValue(value: unknown): string {
  if (value == null) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value) ?? '';
    } catch {
      return 'Unserializable fault detail';
    }
  }
  if (typeof value === 'number' || typeof value === 'boolean' || typeof value === 'bigint')
    return String(value);
  return typeof value;
}
export class FaultPanel {
  constructor(private host: HTMLElement) {
    host.querySelector('h4')!.prepend(icon('alert'), ' ');
    host.querySelector('#error-report')!.prepend(icon('download', 14), ' ');
  }
  show(fault: EngineFault, context: FaultContext) {
    this.host.hidden = false;
    this.host.querySelector('#fault-message')!.textContent =
      fault.kind === 'unsupported-device'
        ? "We don't know how to simulate this camera hardware feature yet. It works fine on a real camera."
        : fault.message;
    const d = fault.details;
    const kind = faultValue(d.type ?? d.error_type ?? fault.kind);
    const access = [
      faultValue(d.operation),
      faultValue(d.address),
      d.value == null ? '' : `← ${faultValue(d.value)}`,
      d.device ? `(${faultValue(d.device)})` : '',
    ]
      .filter(Boolean)
      .join(' ');
    const raw = faultValue(d.error ?? d.message);
    const last = raw.trim().split('\n').at(-1) ?? '';
    const ua = navigator.userAgent,
      browser =
        ua.match(/(?:Chrome|Firefox)\/[\d.]+/)?.[0] ??
        `Safari ${ua.match(/Version\/([\d.]+)/)?.[1] ?? ''}`;
    const os =
      ua.match(/iPhone OS ([\d_]+)/)?.[1]?.replaceAll('_', '.') ??
      ua.match(/Mac OS X ([\d_]+)/)?.[1]?.replaceAll('_', '.') ??
      navigator.platform;
    const lines = [
      `${kind}: ${access || last}`,
      `pc ${faultValue(d.pc ?? 'unknown')} · core ${faultValue(d.cpu ?? 'unknown')} · tick ${faultValue(context.tick ?? d.tick ?? 'unknown')} · firmware ${context.firmware ?? 'unknown'} · build ${context.build} · ${browser} / ${os}`,
    ];
    this.host.querySelector('#fault-raw')!.textContent = lines.join('\n');
  }
  hide() {
    this.host.hidden = true;
  }
}
