import type { EngineConfig } from './engine/api';
export interface UsbEmulator {
  bundle: string;
  module: string;
  scenario: { path: string; sha256: string };
}
export interface Firmware {
  id: string;
  body?: string;
  model: string;
  version: string;
  sha256: string;
  bytes: number;
  download_url: string;
  release_date: string;
  emulate?: Omit<EngineConfig, 'firmwareId'> & { adapter: string; usb?: UsbEmulator };
  model_asset?: string;
}
export interface Catalog {
  schema: number;
  firmwares: Firmware[];
}
export function validateCatalog(value: Catalog): Catalog {
  if (value?.schema !== 1 || !Array.isArray(value.firmwares))
    throw Error('Invalid firmware catalog.');
  const seen = new Set<string>();
  for (const entry of value.firmwares) {
    if (
      !/^[a-z0-9-]+$/.test(entry.id) ||
      typeof entry.body !== 'string' ||
      !/^[a-z0-9-]+$/.test(entry.body) ||
      !/^[a-f0-9]{64}$/.test(entry.sha256) ||
      seen.has(entry.sha256) ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes <= 0 ||
      !entry.model ||
      !entry.version ||
      new URL(entry.download_url).protocol !== 'https:'
    )
      throw Error('Invalid firmware entry.');
    for (const path of [
      entry.emulate?.bundle,
      entry.emulate?.inputs,
      entry.emulate?.usb?.bundle,
      entry.emulate?.usb?.scenario.path,
    ])
      if (path && (!/^[a-zA-Z0-9_./-]+$/.test(path) || path.includes('..') || path.startsWith('/')))
        throw Error('Invalid emulator artifact path.');
    const usb = entry.emulate?.usb;
    if (
      usb &&
      (!/^[a-zA-Z_]\w*(?:\.[a-zA-Z_]\w*)*$/.test(usb.module) ||
        !/^[a-f0-9]{64}$/.test(usb.scenario.sha256))
    )
      throw Error('Invalid USB emulator configuration.');
    seen.add(entry.sha256);
  }
  return value;
}
