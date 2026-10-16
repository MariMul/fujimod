import type { EngineConfig } from './engine/api';
export interface Firmware {
  id: string; body?: string; model: string; version: string; sha256: string; bytes: number;
  download_url: string; release_date: string;
  emulate?: EngineConfig & { adapter: string };
  model_asset?: string;
}
export interface Catalog { schema: number; firmwares: Firmware[] }
export function validateCatalog(value: Catalog): Catalog {
  if (value?.schema !== 1 || !Array.isArray(value.firmwares)) throw Error('Invalid firmware catalog.');
  const seen = new Set<string>();
  for (const entry of value.firmwares) {
    if (!/^[a-z0-9-]+$/.test(entry.id) || (typeof entry.body !== 'string' || !/^[a-z0-9-]+$/.test(entry.body)) ||
      !/^[a-f0-9]{64}$/.test(entry.sha256) || seen.has(entry.sha256) || !Number.isSafeInteger(entry.bytes) || entry.bytes <= 0 ||
      !entry.model || !entry.version || new URL(entry.download_url).protocol !== 'https:') throw Error('Invalid firmware entry.');
    for (const path of [entry.emulate?.bundle, entry.emulate?.inputs]) if (path && (!/^[a-zA-Z0-9_./-]+$/.test(path) || path.includes('..') || path.startsWith('/'))) throw Error('Invalid emulator artifact path.');
    seen.add(entry.sha256);
  }
  return value;
}
