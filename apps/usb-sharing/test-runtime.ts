import { Miniflare } from 'miniflare';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';
execFileSync(
  fileURLToPath(new URL('./node_modules/.bin/wrangler', import.meta.url)),
  ['deploy', '--dry-run', '--outdir', 'build/worker-test'],
  { cwd: fileURLToPath(new URL('.', import.meta.url)), stdio: 'pipe' },
);
const origin = 'https://justjake.github.io';
export function runtime(
  options: {
    bindings?: Record<string, string>;
    outboundService?: (request: import('miniflare').Request) => Promise<Response>;
  } = {},
) {
  return new Miniflare({
    workers: [
      {
        config: {
          name: 'sharing',
          compatibilityDate: '2026-10-17',
          manifest: {
            mainModule: 'worker.mjs',
            modules: {
              'worker.mjs': {
                type: 'esm',
                contents: readFileSync(
                  new URL('./build/worker-test/worker.js', import.meta.url),
                  'utf8',
                ),
              },
            },
          },
          exports: {
            IssueReports: { type: 'durable-object', storage: 'sqlite' },
            ShareRoom: { type: 'durable-object', storage: 'sqlite' },
            TurnBudget: { type: 'durable-object', storage: 'sqlite' },
          },
          env: {
            ISSUES: { type: 'durable-object', worker: 'sharing', exportName: 'IssueReports' },
            ISSUE_REPORTS: { type: 'r2', name: 'reports' },
            ISSUE_LIMIT: {
              type: 'rate-limit',
              namespace: '1002',
              simple: { limit: 5, period: 60 },
            },
            ROOMS: { type: 'durable-object', worker: 'sharing', exportName: 'ShareRoom' },
            TURN_BUDGET: { type: 'durable-object', worker: 'sharing', exportName: 'TurnBudget' },
            CONNECTIONS: {
              type: 'rate-limit',
              namespace: '1001',
              simple: { limit: 60, period: 60 },
            },
            ...Object.fromEntries(
              Object.entries({ ALLOWED_ORIGINS: origin, ...options.bindings }).map(
                ([key, value]) => [key, { type: 'json', value }],
              ),
            ),
          },
        },
        dev: options.outboundService
          ? { outboundService: { type: 'fetcher', handler: options.outboundService } }
          : {},
      },
    ],
  });
}
