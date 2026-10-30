import { test, expect, vi } from 'vite-plus/test';
import worker, { serveRelease, type ReleaseCache, type ReleaseStore } from '../worker.ts';
function harness(fetcher: typeof fetch, known = new Map<string, string>()) {
  const stored = new Map<string, Response>();
  const store: ReleaseStore = {
    get: async (key) => known.get(key) ?? null,
    put: async (key, value) => {
      known.set(key, value);
    },
  };
  const cache: ReleaseCache = {
    match: async (request) => stored.get(request.url)?.clone(),
    put: async (request, response) => {
      stored.set(request.url, response);
    },
  };
  const pending: Promise<unknown>[] = [];
  const context = {
    waitUntil: (promise: Promise<unknown>) => {
      pending.push(promise);
    },
  };
  return {
    pending,
    cache,
    known,
    context,
    request: async (path: string, method = 'GET') => {
      const response = await serveRelease(
        new Request(`https://fujimod.com${path}`, {
          method,
          headers: { Authorization: 'secret', Cookie: 'secret' },
        }),
        cache,
        store,
        context,
        fetcher,
      );
      await Promise.all(pending);
      return response;
    },
  };
}
function release(
  tag: string,
  date: string,
  options: { prerelease?: boolean; draft?: boolean; patch?: boolean } = {},
) {
  return {
    tag_name: tag,
    name: `Release ${tag}`,
    published_at: date,
    draft: options.draft ?? false,
    prerelease: options.prerelease ?? false,
    assets: options.patch === false ? [] : [{ name: 'patches.json', state: 'uploaded' }],
  };
}
test('current selects newest published patch release across pages, includes prereleases and caches for five minutes', async () => {
  const fetcher = vi.fn<typeof fetch>(async (input) =>
    (input instanceof Request ? input.url : input.toString()).endsWith('page=1')
      ? Response.json(
          [
            release('no-patch', '2026-10-27', { patch: false }),
            release('draft', '2026-10-28', { draft: true }),
            release('stable', '2026-10-20'),
          ],
          { headers: { Link: '<https://api.github.com/?page=2>; rel="next"' } },
        )
      : Response.json([release('preview', '2026-10-26', { prerelease: true })]),
  );
  const h = harness(fetcher),
    response = await h.request('/releases/current?ignored=1');
  expect(await response.json()).toEqual({
    tag: 'preview',
    name: 'Release preview',
    published_at: '2026-10-26',
    prerelease: true,
  });
  expect(response.headers.get('cache-control')).toBe('public, max-age=0, s-maxage=300');
  expect((await h.request('/releases/current')).status).toBe(200);
  expect(fetcher).toHaveBeenCalledTimes(2);
  for (const [url, init] of fetcher.mock.calls) {
    expect(url instanceof Request ? url.url : url.toString()).toMatch(
      /^https:\/\/api.github.com\/repos\/justjake\/fujimod\/releases\?/,
    );
    expect(new Headers(init?.headers).has('authorization')).toBe(false);
    expect(new Headers(init?.headers).has('cookie')).toBe(false);
  }
});
test('asset bytes are proxied server-side with a long cache and HEAD shares the cached GET', async () => {
  const bytes = new Uint8Array([1, 2, 3]);
  const fetcher = vi.fn<typeof fetch>(
    async () => new Response(bytes, { headers: { 'Set-Cookie': 'upstream=secret' } }),
  );
  const h = harness(fetcher),
    response = await h.request('/releases/v2/test.fmpatch');
  expect(new Uint8Array(await response.arrayBuffer())).toEqual(bytes);
  expect(response.headers.get('cache-control')).toContain('s-maxage=31536000');
  expect(response.headers.has('set-cookie')).toBe(false);
  expect(fetcher.mock.calls[0][0]).toBe(
    'https://github.com/justjake/fujimod/releases/download/v2/test.fmpatch',
  );
  const head = await h.request('/releases/v2/test.fmpatch', 'HEAD');
  expect(await head.text()).toBe('');
  expect(head.status).toBe(200);
  expect(fetcher).toHaveBeenCalledOnce();
});
test('upstream errors and missing releases are never cached', async () => {
  for (const status of [403, 429, 500, 404]) {
    const fetcher = vi.fn<typeof fetch>(async () => new Response(null, { status }));
    const h = harness(fetcher);
    expect((await h.request('/releases/current')).status).toBe(502);
    expect((await h.request('/releases/v2/notes.md')).status).toBe(status === 404 ? 404 : 502);
    expect(h.pending).toHaveLength(0);
    expect(h.known.size).toBe(0);
  }
  const h = harness(async () => Response.json([]));
  expect((await h.request('/releases/current')).status).toBe(404);
  expect(h.pending).toHaveLength(0);
});
test('current serves the last known release uncached when GitHub refuses', async () => {
  const known = { tag: 'v1', name: 'Release v1', published_at: '2026-10-20', prerelease: true };
  const ok = harness(async () => Response.json([release('v1', '2026-10-20', { prerelease: true })]));
  await ok.request('/releases/current');
  expect(JSON.parse(ok.known.get('current')!)).toEqual(known);
  const limited = harness(async () => new Response(null, { status: 403 }), ok.known),
    response = await limited.request('/releases/current');
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual(known);
  expect(response.headers.get('cache-control')).toBe('no-store');
  expect(limited.pending).toHaveLength(0);
});
test('malformed and traversing paths and non-read methods never reach GitHub', async () => {
  const fetcher = vi.fn<typeof fetch>(),
    h = harness(fetcher);
  for (const path of [
    '/releases/v2/%2fsecret',
    '/releases/v2/%2e%2e%2fsecret',
    '/releases/v2/bad%zz',
    '/releases/v2/file/extra',
  ])
    expect((await h.request(path)).status).toBe(404);
  expect((await h.request('/releases/current', 'POST')).status).toBe(405);
  expect(fetcher).not.toHaveBeenCalled();
});
test('ordinary routes pass to the static asset binding', async () => {
  const fetcher = vi.fn(async () => new Response('static'));
  const response = await worker.fetch(
    new Request('https://fujimod.com/'),
    { ASSETS: { fetch: fetcher }, RELEASES: { get: async () => null, put: async () => {} } },
    { waitUntil() {} },
  );
  expect(await response.text()).toBe('static');
});
