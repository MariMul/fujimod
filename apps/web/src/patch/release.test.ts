import { expect, test, vi } from 'vite-plus/test';
import { loadRelease } from './release.ts';
import { validateIndex, verifyNotes, verifyPatch } from './catalog.ts';
import { sha256 } from './patcher.ts';
const source = 'a'.repeat(64);
async function fixture(installable = false, fpupdate = false) {
  const notes = new TextEncoder().encode('# Release\n\nVerified notes.');
  const program = new Uint8Array(20);
  const identity = { bytes: 1, sha256: source };
  const metadata = new TextEncoder().encode(
    JSON.stringify({
      schema: 1,
      source: { bytes: 10, sha256: source },
      outputs: {
        ffboot: identity,
        ffsys: identity,
        ...(fpupdate ? { fpupdate: identity } : {}),
      },
      commands: 1,
      program_sha256: await sha256(program),
      firmware: 'test-100',
      release: 'v2',
      mods: [],
    }),
  );
  const patch = new Uint8Array(12 + metadata.length + program.length);
  patch.set(new TextEncoder().encode('FJMODP01'));
  new DataView(patch.buffer).setUint32(8, metadata.length, true);
  patch.set(metadata, 12);
  patch.set(program, 12 + metadata.length);
  const index = {
    schema: 2 as const,
    release: 'v2',
    patches: { [source]: 'test.fmpatch' },
    notes: { name: 'notes.md', bytes: notes.length, sha256: await sha256(notes) },
    firmwares: [
      {
        id: 'test-100',
        model: 'Test',
        version: '1.00',
        sha256: source,
        bytes: 10,
        release_date: '2026-01-01',
        download_url: 'https://example.org/',
        patch_sha256: await sha256(patch),
        patch_bytes: patch.length,
        boot_instructions: ['Copy files.'],
        downloads: { sd_card: true, installable },
      },
    ],
  };
  const files = new Map<string, Response>([
    [
      '/releases/current',
      Response.json({
        tag: 'v2',
        name: 'Release 2',
        published_at: '2026-10-26T00:00:00Z',
        prerelease: true,
      }),
    ],
    ['/releases/v2/patches.json', Response.json(index)],
    ['/releases/v2/notes.md', new Response(notes)],
    ['/releases/v2/test.fmpatch', new Response(patch)],
  ]);
  const fetcher = vi.fn<typeof fetch>(
    async (input) =>
      files.get(new URL(input instanceof Request ? input.url : input).pathname)?.clone() ??
      new Response(null, { status: 404 }),
  );
  return { notes, index, patch, fetcher, files };
}
test('runtime release exposes metadata, normalized switches and verified Markdown at same-origin URLs', async () => {
  const f = await fixture();
  const release = await loadRelease('https://fujimod.com/recovery/', f.fetcher);
  expect(release).toMatchObject({
    tag: 'v2',
    name: 'Release 2',
    prerelease: true,
    assetBase: 'https://fujimod.com/releases/v2/',
    notesText: '# Release\n\nVerified notes.',
    firmwares: [{ downloads: { sd_card: true, installable: false } }],
  });
  expect(
    f.fetcher.mock.calls.map(([url]) => (url instanceof Request ? url.url : url.toString())),
  ).toEqual([
    'https://fujimod.com/releases/current',
    'https://fujimod.com/releases/v2/patches.json',
    'https://fujimod.com/releases/v2/notes.md',
  ]);
});
test('missing notes produces no text, and omitted download switches default to false without mutation', async () => {
  const f = await fixture();
  const raw = JSON.parse(JSON.stringify(f.index));
  delete raw.notes;
  delete raw.firmwares[0].downloads;
  const validated = validateIndex(raw);
  expect(validated.firmwares[0].downloads).toEqual({ sd_card: false, installable: false });
  expect(raw.firmwares[0].downloads).toBeUndefined();
  f.files.set('/releases/v2/patches.json', Response.json(raw));
  expect((await loadRelease('https://fujimod.com/', f.fetcher))?.notesText).toBeUndefined();
});
test('rejects old schemas, unsafe notes names, sizes, hashes and malformed download switches', async () => {
  const { index } = await fixture();
  for (const notes of [
    null,
    { ...index.notes, name: '../notes.md' },
    { ...index.notes, name: 'patches.json' },
    { ...index.notes, bytes: -1 },
    { ...index.notes, bytes: 1024 ** 2 + 1 },
    { ...index.notes, sha256: 'bad' },
  ])
    expect(() => validateIndex({ ...index, notes })).toThrow();
  expect(() => validateIndex({ ...index, schema: 1 })).toThrow();
  for (const downloads of [null, [], { sd_card: 'yes' }, { installable: 1 }])
    expect(() =>
      validateIndex({ ...index, firmwares: [{ ...index.firmwares[0], downloads }] }),
    ).toThrow();
});
test('notes reject changed content, length and invalid UTF-8', async () => {
  const { index, notes, files, fetcher } = await fixture();
  const changed = notes.slice();
  changed[0] ^= 1;
  await expect(verifyNotes(index.notes, changed)).rejects.toThrow(/integrity/);
  await expect(verifyNotes(index.notes, notes.slice(1))).rejects.toThrow(/integrity/);
  const invalid = new Uint8Array([255]);
  await expect(
    verifyNotes({ name: 'notes.md', bytes: 1, sha256: await sha256(invalid) }, invalid),
  ).rejects.toThrow();
  files.set('/releases/v2/notes.md', new Response(changed));
  await expect(loadRelease('https://fujimod.com/', fetcher)).rejects.toThrow(/integrity/);
});
test('installable requires a verified FPUPDATE output in both release loading and standalone verification', async () => {
  const bad = await fixture(true);
  await expect(
    verifyPatch(validateIndex(bad.index), bad.index.firmwares[0], bad.patch),
  ).rejects.toThrow(/index/);
  await expect(loadRelease('https://fujimod.com/', bad.fetcher)).rejects.toThrow(/index/);
  const good = await fixture(true, true);
  expect(
    (await loadRelease('https://fujimod.com/', good.fetcher))?.firmwares[0].downloads.installable,
  ).toBe(true);
});
test('no current release is allowed, but upstream errors and tag mismatches fail', async () => {
  const f = await fixture();
  f.files.set('/releases/current', new Response(null, { status: 404 }));
  expect(await loadRelease('https://fujimod.com/', f.fetcher)).toBeUndefined();
  f.files.set('/releases/current', new Response(null, { status: 502 }));
  await expect(loadRelease('https://fujimod.com/', f.fetcher)).rejects.toThrow(/unavailable/);
  f.files.set(
    '/releases/current',
    Response.json({ tag: 'v2', name: 'New', published_at: '2026-10-26', prerelease: false }),
  );
  f.files.set('/releases/v2/patches.json', Response.json({ ...f.index, release: 'different' }));
  await expect(loadRelease('https://fujimod.com/', f.fetcher)).rejects.toThrow(/tag/);
});
