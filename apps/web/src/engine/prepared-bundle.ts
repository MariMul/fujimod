/** Load the explicit cold media inputs of a native prepared camera bundle. */
export async function loadPreparedBundle(prepared: unknown, base: URL) {
  if (prepared === null || typeof prepared !== 'object')
    throw Error('Prepared camera has no bundle files.');
  const path = (key: string) => {
    if (!(key in prepared)) throw Error(`Prepared camera has no ${key} input.`);
    const value: unknown = Reflect.get(prepared, key);
    if (typeof value !== 'string' || value.length === 0)
      throw Error(`Prepared camera has an invalid ${key} input.`);
    return new URL(value, base);
  };
  const urls = ['document', 'bootRom'].map(path);
  const optional = (key: string) => (key in prepared ? path(key) : undefined);
  const nandUrl = optional('nand'),
    settingsUrl = optional('settings');
  const [document, bootRom, nand, settings] = await Promise.all(
    [...urls, nandUrl, settingsUrl].map(async (url) => {
      if (!url) return undefined;
      const response = await fetch(url);
      if (!response.ok) throw Error(`Prepared camera input is unavailable: ${url.pathname}`);
      return response;
    }),
  );
  if (!document || !bootRom) throw Error('Prepared camera inputs are incomplete.');
  if (nand && !nand.body) throw Error('Prepared camera NAND input is empty.');
  const [text, bootRomBytes, nandBytes, settingsBytes] = await Promise.all([
    document.text(),
    bootRom.arrayBuffer(),
    nand?.body
      ? new Response(nand.body.pipeThrough(new DecompressionStream('gzip'))).arrayBuffer()
      : undefined,
    settings?.arrayBuffer(),
  ]);
  const inputs = {
    bootRom: new Uint8Array(bootRomBytes),
    nand: nandBytes ? new Uint8Array(nandBytes) : undefined,
    settings: settingsBytes ? new Uint8Array(settingsBytes) : undefined,
  };
  return { text, inputs };
}
