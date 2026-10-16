/** FJMODP01 interpreter. No camera/model maps, network access or dependencies. */
export const MAGIC = 'FJMODP01';
const names = ['ffboot', 'ffsys'];
const fail = message => { throw new Error(message); };
const hashPattern = /^[0-9a-f]{64}$/;
const integer = (value, max) => Number.isInteger(value) && value > 0 && value <= max;
export async function bytes(value) {
  if (value instanceof Uint8Array) return value;
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (typeof value?.arrayBuffer === 'function') return new Uint8Array(await value.arrayBuffer());
  return fail('Expected a File, Blob, ArrayBuffer or Uint8Array.');
}
export async function sha256(value) {
  const digest = await crypto.subtle.digest('SHA-256', await bytes(value));
  return Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, '0')).join('');
}
export function inspectPatch(input) {
  const data = input;
  if (!(data instanceof Uint8Array) || data.length < 12 || new TextDecoder().decode(data.subarray(0, 8)) !== MAGIC)
    return fail('This is not a supported Fujimod patch.');
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const length = view.getUint32(8, true);
  if (!integer(length, 1048576) || length + 12 > data.length) return fail('Invalid patch metadata length.');
  const meta = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(data.subarray(12, 12 + length)));
  if (meta.schema !== 1 || !integer(meta.source?.bytes, 256 * 1024 * 1024) || !hashPattern.test(meta.source.sha256) ||
      !integer(meta.commands, 1000000) || !hashPattern.test(meta.program_sha256)) return fail('Invalid patch metadata.');
  for (const name of names)
    if (!integer(meta.outputs?.[name]?.bytes, 128 * 1024 * 1024) || !hashPattern.test(meta.outputs[name].sha256))
      return fail('Invalid patch output metadata.');
  return { meta, program: data.subarray(12 + length) };
}
function decodeLZ(data, length) {
  if (data.length < 20) return fail('Truncated LZ header.');
  const header = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const packed = header.getUint32(4, true);
  if (header.getUint32(0, true) !== length || packed !== data.length || packed < 20 ||
      header.getUint32(12, true) !== 0x4000 || header.getUint32(16, true) !== 4 ||
      header.getUint32(8, true) !== Math.ceil(packed / 0x4000)) return fail('Invalid LZ stream header.');
  const output = new Uint8Array(length);
  let at = 20, written = 0;
  while (at < packed) {
    const command = data[at++];
    if (command < 128) {
      if (!command || at + command > packed || written + command > length) return fail('Invalid LZ literal.');
      output.set(data.subarray(at, at + command), written);
      at += command; written += command;
    } else {
      if (at >= packed) return fail('Truncated LZ match.');
      const second = data[at++], distance = ((command & 127) << 4) | (second >> 4), count = second & 15;
      if (!distance || distance > written || !count || written + count > length) return fail('Invalid LZ match.');
      for (let n = 0; n < count; n++) { output[written] = output[written - distance]; written++; }
    }
  }
  if (written !== length) return fail('LZ output length mismatch.');
  return output;
}
/** async patch(baseStockDatFile, compiledPatch) -> {ffboot, ffsys, metadata}. */
export async function patch(baseStockDatFile, compiledPatch) {
  const [base, data] = await Promise.all([bytes(baseStockDatFile), bytes(compiledPatch)]);
  const { meta, program } = inspectPatch(data);
  if (base.length !== meta.source.bytes || await sha256(base) !== meta.source.sha256)
    return fail('Stock DAT SHA-256 does not match this patch.');
  if (await sha256(program) !== meta.program_sha256) return fail('Patch program hash mismatch.');
  const outputs = names.map(name => new Uint8Array(meta.outputs[name].bytes));
  const view = new DataView(program.buffer, program.byteOffset, program.byteLength);
  let cursor = 0;
  for (let n = 0; n < meta.commands; n++) {
    if (cursor + 20 > program.length) return fail('Truncated patch command.');
    const op = program[cursor], target = program[cursor + 1], flags = program[cursor + 2], reserved = program[cursor + 3];
    const dest = view.getUint32(cursor + 4, true), length = view.getUint32(cursor + 8, true);
    const source = view.getUint32(cursor + 12, true), packed = view.getUint32(cursor + 16, true);
    cursor += 20;
    if (target > 1 || flags > 1 || reserved || !length || dest + length > outputs[target].length)
      return fail('Invalid patch command bounds or flags.');
    let chunk;
    if (op === 1 || op === 2) {
      if (!packed || source + packed > base.length || (op === 1 && packed !== length)) return fail('Invalid patch source bounds.');
      chunk = base.subarray(source, source + packed);
      if (flags) chunk = chunk.map(value => value ^ 255);
      if (op === 2) chunk = decodeLZ(chunk, length);
    } else if (op === 3) {
      if (flags || source || packed || cursor + length > program.length) return fail('Invalid patch literal.');
      chunk = program.subarray(cursor, cursor + length); cursor += length;
    } else return fail('Unknown patch opcode.');
    outputs[target].set(chunk, dest);
  }
  if (cursor !== program.length) return fail('Trailing patch bytes.');
  for (let n = 0; n < names.length; n++)
    if (await sha256(outputs[n]) !== meta.outputs[names[n]].sha256) return fail('Patch output hash mismatch.');
  return { ffboot: outputs[0], ffsys: outputs[1], metadata: meta };
}
