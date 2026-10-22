
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
  const urls = ['document', 'bootRom', 'nand', 'settings'].map(path);
  const [document, bootRom, nand, settings] = await Promise.all(
    urls.map(async (url) => {
      const response = await fetch(url);
      if (!response.ok) throw Error(`Prepared camera input is unavailable: ${url.pathname}`);
      return response;
    }),
  );
  if (!document || !bootRom || !nand || !settings) throw Error('Prepared camera inputs are incomplete.');
  if (!nand.body) throw Error('Prepared camera NAND input is empty.');
  const [text, bootRomBytes, nandBytes, settingsBytes] = await Promise.all([
    document.text(),
    bootRom.arrayBuffer(),
    new Response(nand.body.pipeThrough(new DecompressionStream('gzip'))).arrayBuffer(),
    settings.arrayBuffer(),
  ]);
  const inputs = {
    bootRom: new Uint8Array(bootRomBytes),
    nand: new Uint8Array(nandBytes),
    settings: new Uint8Array(settingsBytes),
  };
  return { text, inputs };
}
