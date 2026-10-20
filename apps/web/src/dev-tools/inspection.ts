/** Read-only ff80 operations, shared by USB and the simulator worker. */
export const infoSelectors = [
  'Vendor',
  'Manufacturer',
  'Product',
  'InternalVersion',
  'DeviceType',
  'Serial',
  'Framework',
] as const;
export type InfoSelector = (typeof infoSelectors)[number];
export type InspectionRequest =
  | { kind: 'identity'; selector: InfoSelector }
  | { kind: 'geometry' }
  | { kind: 'configuration'; offset: number; length: number }
  | { kind: 'memory'; address: number; length: number };
export interface InspectionReply {
  data: Uint8Array;
  text?: string;
  capacity?: number;
  pageBytes?: number;
}
export type InspectorRead = (request: InspectionRequest) => Promise<InspectionReply>;
export const inspectorReadLimit = 16384;

export function memoryRange(address: string, length: string) {
  const integer = (value: string) =>
    /^(?:0x[\da-f]+|\d+)$/i.test(value.trim()) ? Number(value) : NaN;
  const start = integer(address),
    count = integer(length);
  if (
    !Number.isSafeInteger(start) ||
    start < 0 ||
    start > 0xffffffff ||
    !Number.isSafeInteger(count) ||
    count < 1 ||
    count > inspectorReadLimit ||
    start + count > 0x100000000
  )
    throw Error(`Enter a 32-bit address and a length from 1 to ${inspectorReadLimit} bytes.`);
  return { address: start, length: count };
}
