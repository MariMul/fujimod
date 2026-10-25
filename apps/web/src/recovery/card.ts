/** One file of an SD recovery card. */
export interface RecoveryFile {
  name: string;
  data: Uint8Array<ArrayBuffer>;
}

/** The SD recovery card files for `firmwareId`, built from its official update `dat`. */
export function recoveryCard(_firmwareId: string, _dat: Uint8Array): Promise<RecoveryFile[]> {
  return Promise.reject(Error('Recovery files are not built yet.'));
}
