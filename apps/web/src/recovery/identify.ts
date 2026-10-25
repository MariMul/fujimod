import { catalog, type PhysicalControls } from 'fujimod';
import { validateCatalog, type Catalog, type Firmware } from '../catalog';
import { bootCombination, describeBootHold } from '../cameras/boot-options';

export interface IdentifiedFirmware {
  firmware: Firmware;
  /** How to hold the camera to boot from the SD card, then to enter firmware update. */
  sdBoot: string[];
  firmwareUpdate: string[];
}

declare const __FUJIMOD_CATALOG__: string;
async function wholeCatalog(): Promise<Catalog> {
  const response = await fetch(new URL(__FUJIMOD_CATALOG__, document.baseURI));
  if (!response.ok) throw Error('Could not load the firmware catalog.');
  return validateCatalog((await response.json()) as Catalog);
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes as unknown as ArrayBuffer);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

function holds(controls: PhysicalControls, mode: 'sd' | 'firmware_update'): string[] {
  const combination = bootCombination(controls, mode);
  const steps = combination.hold.map((input) => describeBootHold(controls, input));
  if (combination.usb === false) steps.unshift('Unplug USB.');
  return steps;
}

/** Identify the official update `dat` by content hash and read its boot combinations. */
export async function identify(dat: Uint8Array): Promise<IdentifiedFirmware> {
  if (!dat.length || dat.length > 256 * 1024 * 1024)
    throw Error('Choose a firmware DAT smaller than 256 MB.');
  const hash = await sha256(dat);
  const index = await wholeCatalog();
  const firmware = index.firmwares.find((entry) => entry.sha256 === hash);
  if (!firmware) throw Error(`Unrecognized firmware · SHA-256 ${hash}`);
  if (dat.length !== firmware.bytes) throw Error('Firmware size differs from the catalog.');
  const controls = catalog
    .selections()
    .find((selection) => selection.firmware.id === firmware.id)?.physical_controls;
  if (!controls?.boot?.sd)
    throw Error(`SD card recovery is not available for the ${firmware.model} yet.`);
  return {
    firmware,
    sdBoot: holds(controls, 'sd'),
    firmwareUpdate: holds(controls, 'firmware_update'),
  };
}
