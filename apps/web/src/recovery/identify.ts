import { catalog as reference, recovery, type PhysicalControls } from 'fujimod';
import { describeBootHold } from '../cameras/boot-options';
import type { DownloadRow } from '../cameras/FirmwareDownloads';

/** An official release from the catalog's firmware versions. */
export interface Release {
  id: string;
  model: string;
  version: string;
}

export interface IdentifiedFirmware {
  firmware: Release;
  /** How to hold the camera to boot from the SD card, when the catalog documents it. */
  sdBoot?: string[];
}

function controlsFor(firmwareId: string): PhysicalControls | undefined {
  return (
    reference.selections().find((selection) => selection.firmware.id === firmwareId)
      ?.physical_controls ?? undefined
  );
}

/** Every camera with an official release in the catalog, newest release each, by name, GFX last. */
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
  const gfx = (row: DownloadRow) => (row.model.startsWith('GFX') ? 1 : 0);
  return [...newest.values()].sort(
    (a, b) => gfx(a) - gfx(b) || a.model.localeCompare(b.model, 'en', { numeric: true }),
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
  const release = reference.firmwareVersions().find((entry) => entry.update_sha256 === hash);
  if (!release) throw Error(`Unrecognized firmware · SHA-256 ${hash}`);
  const firmware = { id: release.firmware_id, model: release.product, version: release.version };
  const status = recovery.availability(firmware.id);
  let sd;
  try {
    if (!status.canBuild)
      throw Error(
        `Recovery files can't be built for the ${firmware.model} yet. Missing: ${status.missing.join(', ')}.`,
      );
    sd = status.sdBoot;
  } finally {
    status.free();
  }
  const controls = controlsFor(firmware.id);
  return {
    firmware,
    sdBoot: sd && controls ? sd.hold.map((input) => describeBootHold(controls, input)) : undefined,
  };
}
