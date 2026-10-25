import { catalog as reference, type PhysicalControls } from 'fujimod';
import { validateCatalog, type Catalog, type Firmware } from '../catalog';
import { bootCombination, describeBootHold } from '../cameras/boot-options';
import type { DownloadRow } from '../cameras/FirmwareDownloads';

export interface IdentifiedFirmware {
  firmware: Firmware;
  /** How to hold the camera to boot from the SD card. */
  sdBoot: string[];
}

declare const __FUJIMOD_INITIAL_CATALOG__: Catalog;
const catalog = validateCatalog(__FUJIMOD_INITIAL_CATALOG__);

function controlsFor(firmwareId: string): PhysicalControls | undefined {
  return (
    reference.selections().find((selection) => selection.firmware.id === firmwareId)
      ?.physical_controls ?? undefined
  );
}

/** Every camera with an official release in the catalog, newest release each, by name. */
export function recoveryCameras(): DownloadRow[] {
  const newest = new Map<string, DownloadRow>();
  for (const release of reference.firmwareVersions()) {
    const known = newest.get(release.product);
    if (
      known?.version &&
      known.version.localeCompare(release.version, 'en', { numeric: true }) >= 0
    )
      continue;
    newest.set(release.product, {
      model: release.product,
      version: release.version,
      url: release.download_url,
    });
  }
  return [...newest.values()].sort((a, b) =>
    a.model.localeCompare(b.model, 'en', { numeric: true }),
  );
}

async function sha256(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** Identify the official update `dat` by content hash and read its SD boot combination. */
export async function identify(dat: Uint8Array<ArrayBuffer>): Promise<IdentifiedFirmware> {
  if (!dat.length || dat.length > 256 * 1024 * 1024)
    throw Error('Choose a firmware DAT smaller than 256 MB.');
  const hash = await sha256(dat);
  const firmware = catalog.firmwares.find((entry) => entry.sha256 === hash);
  if (!firmware) throw Error(`Unrecognized firmware · SHA-256 ${hash}`);
  if (dat.length !== firmware.bytes) throw Error('Firmware size differs from the catalog.');
  const controls = controlsFor(firmware.id);
  if (!controls?.boot?.sd)
    throw Error(`SD card recovery is not available for the ${firmware.model} yet.`);
  const sd = bootCombination(controls, 'sd');
  return { firmware, sdBoot: sd.hold.map((input) => describeBootHold(controls, input)) };
}
