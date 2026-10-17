import { expect, test } from 'vite-plus/test';
import { gunzipSync } from 'node:zlib';
import { prepareDownload } from './download';
import { tar } from './fixtures.test-helper';
test('download gzip preserves the exact camera TAR and uses an accepted extension', async () => {
  const ready = await prepareDownload(new Blob([tar]), 'R1234567.TAR');
  expect(ready.name).toBe('R1234567.tar.gz');
  expect(Uint8Array.from(gunzipSync(new Uint8Array(await ready.blob.arrayBuffer())))).toEqual(tar);
});
