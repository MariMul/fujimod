/** RAM captures through a firmware's catalog bulk path, and their tar archive. */
import { catalog, type CaptureScope, type ResolvedSelection } from 'fujimod';
import type { Camera, CameraIdentity, ToolSession } from '../camera-model';
import type { ControlDiff } from './mapping';
import { TarArchive } from './tar';

/** Bytes requested per qualified bulk read. */
export const PIECE_BYTES = 16 << 20;
export const MIB = 1 << 20;

/** The catalog firmware whose located bulk path matches the camera's reported identity. */
export function bulkSelection(
  identity: CameraIdentity | undefined,
  selections: readonly ResolvedSelection[] = catalog.selections(),
): ResolvedSelection | undefined {
  const product = identity?.product?.trim();
  const version = identity?.internalVersion?.trim();
  if (!product || !version) return undefined;
  return selections.find(
    (selection) =>
      selection.bulk?.product === product && selection.bulk.internal_version === version,
  );
}

export function scopeBytes(scope: CaptureScope) {
  return scope.ranges.reduce((total, range) => total + range.size, 0);
}

export interface Piece {
  offset: number;
  size: number;
  sha256: string;
}
export interface RangeCapture {
  name: string;
  address: number;
  size: number;
  parts: Uint8Array<ArrayBuffer>[];
  pieces: Piece[];
}
export interface StateCapture {
  state: string;
  label: string;
  scope: string;
  time: string;
  ranges: RangeCapture[];
  overwritten: { address: number; size: number }[];
  photo?: File;
}
export interface ControlMapping {
  /** Catalog control ID, when one was chosen. */
  control?: string;
  name: string;
  note: string;
  time: string;
  scope: string;
  address: number;
  size: number;
  diff: ControlDiff;
}
export interface CaptureSet {
  firmwareId: string;
  states: StateCapture[];
  mappings: ControlMapping[];
}

async function sha256(data: Uint8Array<ArrayBuffer>) {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', data));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** Read every range in pieces, reporting bytes done of the total. */
export async function captureRanges(
  session: ToolSession,
  firmwareId: string,
  scope: CaptureScope,
  progress: (done: number, total: number) => void,
  pieceBytes = PIECE_BYTES,
): Promise<Pick<StateCapture, 'ranges' | 'overwritten'>> {
  const total = scopeBytes(scope);
  let done = 0;
  const overwritten = new Map<string, { address: number; size: number }>();
  const ranges: RangeCapture[] = [];
  progress(0, total);
  for (const range of scope.ranges) {
    const capture: RangeCapture = {
      name: range.name,
      address: range.address,
      size: range.size,
      parts: [],
      pieces: [],
    };
    ranges.push(capture);
    for (let offset = 0; offset < range.size; offset += pieceBytes) {
      const size = Math.min(pieceBytes, range.size - offset);
      const read = await session.readBulk(firmwareId, range.address + offset, size);
      if (read.data.length !== size) throw Error('Bulk read returned a partial piece');
      const data = new Uint8Array(read.data);
      capture.parts.push(data);
      capture.pieces.push({ offset, size, sha256: await sha256(data) });
      if (read.overwritten)
        overwritten.set(`${read.overwritten.address}:${read.overwritten.size}`, read.overwritten);
      done += size;
      progress(done, total);
    }
  }
  return { ranges, overwritten: [...overwritten.values()] };
}

/** One contiguous copy of a single-range capture, for comparisons. */
export function contiguous(capture: Pick<StateCapture, 'ranges'>): Uint8Array<ArrayBuffer> {
  if (capture.ranges.length !== 1) throw Error('Expected one captured range');
  const [range] = capture.ranges;
  const bytes = new Uint8Array(range.size);
  let at = 0;
  for (const part of range.parts) {
    bytes.set(part, at);
    at += part.length;
  }
  return bytes;
}

function slug(text: string) {
  return (
    text
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')
      .slice(0, 40) || 'capture'
  );
}
const hex = (value: number) => `0x${value.toString(16)}`;

/** Store-only tar: manifest.json, each range's bytes and photos, mapping words as binary. */
export function captureArchive(camera: Camera, set: CaptureSet, createdAt = new Date()): Blob {
  const files: [string, Blob | readonly Uint8Array<ArrayBuffer>[]][] = [];
  const states = set.states.map((capture, index) => {
    const directory = `states/${String(index + 1).padStart(3, '0')}-${slug(capture.label)}`;
    const ranges = capture.ranges.map((range) => {
      const file = `${directory}/${range.name}-${hex(range.address)}.bin`;
      files.push([file, range.parts]);
      return {
        name: range.name,
        address: range.address,
        size: range.size,
        file,
        pieces: range.pieces,
      };
    });
    let photo;
    if (capture.photo) {
      const extension = capture.photo.name.split('.').pop()?.toLowerCase() ?? 'jpg';
      photo = `${directory}/photo.${extension}`;
      files.push([photo, capture.photo]);
    }
    return {
      state: capture.state,
      label: capture.label,
      scope: capture.scope,
      time: capture.time,
      ranges,
      overwritten: capture.overwritten,
      photo,
    };
  });
  const mappings = set.mappings.map((mapping, index) => {
    const base = `mappings/${String(index + 1).padStart(3, '0')}-${slug(mapping.name)}`;
    const candidates = `${base}-candidates.u32le`;
    const changed = `${base}-changed.u32le`;
    files.push([candidates, [new Uint8Array(mapping.diff.candidates.buffer as ArrayBuffer)]]);
    files.push([changed, [new Uint8Array(mapping.diff.changed.buffer as ArrayBuffer)]]);
    return {
      control: mapping.control ?? null,
      name: mapping.name,
      note: mapping.note,
      time: mapping.time,
      scope: mapping.scope,
      address: mapping.address,
      size: mapping.size,
      candidates: { file: candidates, count: mapping.diff.candidates.length },
      changed: {
        file: changed,
        format: 'address, released, held, released, held',
        count: mapping.diff.changed.length / 5,
        omitted: mapping.diff.omitted,
      },
    };
  });
  const manifest = {
    schema: 'fujimod-state-captures-1',
    createdAt: createdAt.toISOString(),
    camera: { name: camera.name, transport: camera.transport, identity: camera.identity ?? {} },
    firmware_id: set.firmwareId,
    states,
    mappings,
  };
  const tar = new TarArchive(createdAt.getTime());
  tar.add('manifest.json', new TextEncoder().encode(JSON.stringify(manifest, null, 2)));
  for (const [name, data] of files) tar.add(name, data);
  return tar.blob();
}
