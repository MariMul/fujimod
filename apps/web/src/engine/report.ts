import { zipFiles } from '../archive.ts';

export interface ReportSubmission {
  description: string;
  issue?: { url: string; fields: Record<string, string> };
}

/** Complete diagnostic metadata remains readable even if the worker has failed. */
export function errorReportZip(report: Record<string, unknown>, checkpoint?: ArrayBuffer): Blob {
  const text = (value: string) => new TextEncoder().encode(value);
  const files: [string, Uint8Array<ArrayBuffer>][] = [
    ['report.json', text(JSON.stringify(report, null, 2) + '\n')],
    ['README.txt', text('The snapshot contains guest RAM, CPU/device state and host input queues.\n' +
      'Use the matching emulator sources and firmware identified in report.json.\n' +
      'Python: session = native.web.recovery.restore_session(snapshot_bytes)\n' +
      'snapshot_bytes is the snapshot.zip member, without rebasing the RTC.\n' +
      'The snapshot ticks and any unavailable/skipped points are recorded in report.json.\n' +
      'Recent inputs have guest timestamps; the failing request is recorded separately.\n')],
  ];
  if (checkpoint) files.push(['snapshot.zip', new Uint8Array(checkpoint)]);
  return zipFiles(files);
}
