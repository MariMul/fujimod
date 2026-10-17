import type { EngineFault } from '../engine/fault';

export interface SimulatorIssueContext {
  event: 'feedback' | 'crash';
  camera?: { model: string; version: string; id: string };
  fault?: EngineFault;
}

/** Both entry points use the same template, with context captured at report time. */
export function simulatorIssueURL(description: string, context: SimulatorIssueContext): string {
  const url = new URL('https://github.com/justjake/fujimod/issues/new');
  const labels = ['Simulator'];
  if (context.event === 'crash') labels.push('bug');
  if (context.camera) labels.push(`Camera: ${context.camera.model}`);
  url.searchParams.set('template', 'simulator.md');
  url.searchParams.set('labels', labels.join(','));
  const camera = context.camera;
  url.searchParams.set('title', `[Simulator${camera ? ` · ${camera.model}` : ''}] ${context.event === 'crash' ? 'Crash' : 'Feedback'}`);
  const sections = [
    `## Feedback\n\n${description.trim()}`,
    `## Simulator\n\nReport: ${context.event === 'crash' ? 'Crash' : 'Feedback'}\nCamera: ${camera?.model ?? 'Not identified'}\nFirmware: ${camera ? `${camera.version} (${camera.id})` : 'Not identified'}`,
  ];
  if (context.event === 'crash' && context.fault) {
    const fault = context.fault;
    // Keep the full diagnostic data in the ZIP; include only a brief summary in the URL.
    const details = ['type', 'device', 'operation', 'address', 'pc', 'cpu']
      .filter(key => fault.details[key] != null)
      .map(key => `${key}: ${String(fault.details[key]).slice(0, 160)}`);
    sections.push(`## Crash\n\n${fault.kind}: ${fault.message}\n${details.join('\n')}`);
  }
  sections.push('## Simulator report\n\nDrag and drop the downloaded simulator report ZIP here to upload it before submitting this issue.');
  url.searchParams.set('body', sections.join('\n\n') + '\n');
  return url.href;
}
