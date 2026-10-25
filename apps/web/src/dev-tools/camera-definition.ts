import { validateCatalog, type Catalog } from '../catalog';

declare const __FUJIMOD_CATALOG__: string;
/** Load the camera workflow table selected by a checked firmware identity. */
export async function cameraDefinition(firmwareId: string) {
  const response = await fetch(new URL(__FUJIMOD_CATALOG__, document.baseURI));
  if (!response.ok) throw Error('Could not load the firmware catalog.');
  const catalog = validateCatalog((await response.json()) as Catalog);
  const firmware = catalog.firmwares.find((entry) => entry.id === firmwareId);
  if (!firmware?.camera_table) throw Error('The selected firmware has no camera table.');
  const table = await fetch(new URL(firmware.camera_table, document.baseURI));
  if (!table.ok) throw Error('Could not load the camera table.');
  return [await table.text(), firmware.id, firmware.sha256] as const;
}
