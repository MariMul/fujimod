import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { deliverReport } from '../src/ui/report-delivery.ts';

const report = {
  blob: new Blob(['ZIP']),
  name: 'report.zip',
  issueURL: 'https://github.com/justjake/fujimod/issues/new',
};
test('download starts before GitHub is opened, including a blocked popup', () => {
  const events = [];
  globalThis.window = {
    open: (...args) => {
      events.push(['open', ...args]);
      return null;
    },
  };
  deliverReport(report, (...args) => events.push(['download', ...args]));
  assert.deepEqual(events, [
    ['download', report.blob, report.name],
    ['open', report.issueURL, '_blank', 'noopener,noreferrer'],
  ]);
});
test('a download failure prevents GitHub navigation', () => {
  globalThis.window = { open: () => assert.fail('must not navigate') };
  assert.throws(
    () =>
      deliverReport(report, () => {
        throw Error('download failed');
      }),
    /download failed/,
  );
});
