import { recovery } from 'fujimod';

/** One file of an SD recovery card. */
export interface RecoveryFile {
  name: string;
  data: Uint8Array<ArrayBuffer>;
}

export type RecoveryDownload = 'system' | 'green-led';

/** The SD recovery card files for `firmwareId`, built from its official update `dat`. */
export function recoveryCard(
  firmwareId: string,
  dat: Uint8Array,
  kind: RecoveryDownload,
): RecoveryFile[] {
  const card =
    kind === 'green-led'
      ? recovery.buildGreenLed(firmwareId, dat)
      : recovery.build(firmwareId, dat);
  try {
    return card.takeFiles().map((file) => {
      try {
        return { name: file.name, data: new Uint8Array(file.bytes) };
      } finally {
        file.free();
      }
    });
  } finally {
    card.free();
  }
}
