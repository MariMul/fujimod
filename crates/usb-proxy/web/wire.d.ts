export const VERSION: 2;
export const MAX_TRANSFER: number;
export const MAX_HEADER: number;
export const MAX_MESSAGE: number;
export function encodeFrame(frame: object): Uint8Array;
export function decodeFrame(bytes: ArrayBuffer | Uint8Array): any;
