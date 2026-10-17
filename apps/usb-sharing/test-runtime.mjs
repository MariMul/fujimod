import { Miniflare } from 'miniflare';
import { readFileSync } from 'node:fs';
const origin = 'https://justjake.github.io';
export function runtime(options = {}) {
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
                contents: readFileSync(new URL('./worker.mjs', import.meta.url), 'utf8'),
              },
            },
          },
          exports: {
            ShareRoom: { type: 'durable-object', storage: 'sqlite' },
            TurnBudget: { type: 'durable-object', storage: 'sqlite' },
          },
          env: {
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
