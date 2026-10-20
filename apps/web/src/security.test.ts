import { test } from 'vitest';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { contentSecurityPolicy } from './security';
import middleware from './middleware';
test('SSR and static hosting enforce the same Wasm policy', async () => {
  const response = await middleware(
    { request: new Request('https://example.invalid'), locals: {} },
    async () => new Response(''),
  );
  assert.equal(response.headers.get('content-security-policy'), contentSecurityPolicy);
  const headers = await readFile(new URL('../public/_headers', import.meta.url), 'utf8');
  assert.equal(headers.trim(), '/*\n  Content-Security-Policy: ' + contentSecurityPolicy);
  assert.ok(!contentSecurityPolicy.includes("'unsafe-eval'"));
});
