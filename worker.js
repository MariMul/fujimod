import { patch, sha256 } from './patcher.js';
import { validateIndex, verifyPatch } from './catalog.js';
self.onmessage = async ({ data: { file, index, artifactBase } }) => {
  try {
    validateIndex(index);
    self.postMessage({ stage: 'Checking firmware…' });
    const base = new Uint8Array(await file.arrayBuffer());
    const digest = await sha256(base);
    const firmware = index.firmwares.find(entry => entry.sha256 === digest);
    if (!firmware) { self.postMessage({ unsupported: true, digest }); return; }
    if (base.length !== firmware.bytes) throw new Error('Firmware size does not match its release entry.');
    self.postMessage({ stage: `Building ${firmware.model} ${firmware.version} SD boot files…` });
    const response = await fetch(new URL(index.patches[digest], artifactBase), { cache: 'no-cache' });
    if (!response.ok) throw new Error('The release patch could not be downloaded. Please try again.');
    const compiled = new Uint8Array(await response.arrayBuffer());
    await verifyPatch(index, firmware, compiled);
    const result = await patch(base, compiled);
    self.postMessage({ result, firmware }, [result.ffboot.buffer, result.ffsys.buffer]);
  } catch (error) { self.postMessage({ error: error.message }); }
};
