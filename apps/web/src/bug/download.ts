import { zipFiles } from '@fujimod/emulator-ui/archive';
/** Preserve the camera TAR byte-for-byte inside a GitHub-compatible archive. */
export async function prepareDownload(
  archive: Blob,
  name: string,
): Promise<{ blob: Blob; name: string }> {
  if (typeof CompressionStream !== 'undefined') {
    const blob = await new Response(
      archive.stream().pipeThrough(new CompressionStream('gzip')),
    ).blob();
    return { blob, name: name.replace(/\.tar$/i, '.tar.gz') };
  }
  return {
    blob: zipFiles([[name, new Uint8Array(await archive.arrayBuffer())]]),
    name: name.replace(/\.tar$/i, '.tar.zip'),
  };
}
