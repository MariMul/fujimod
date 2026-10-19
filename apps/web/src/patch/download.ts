import type { PatchFiles } from './patcher.ts';
import { zipFiles } from '../archive.ts';

export function cardZip({ ffboot, ffsys }: PatchFiles): Blob {
  return zipFiles([
    ['ffboot.bin', ffboot],
    ['ffsys.bin', ffsys],
  ]);
}
