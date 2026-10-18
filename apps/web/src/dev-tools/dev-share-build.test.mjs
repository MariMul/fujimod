import { test, expect } from 'vite-plus/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

for (const mode of ['production', 'local-preview']) {
  test(`${mode} bundle ${mode === 'production' ? 'eliminates' : 'includes'} automatic sharing`, async () => {
    // Isolate Vite from Vitest's NODE_ENV=test: build with real production constants.
    const script = `
      import { build } from 'vite';
      import solid from '@solidjs/vite-plugin';
      const result = await build({
        configFile: false, mode: ${JSON.stringify(mode)}, logLevel: 'silent',
        plugins: [solid({ hot: false })],
        build: {
          write: false, minify: true,
          rollupOptions: {
            input: ${JSON.stringify(fileURLToPath(new URL('./Page.tsx', import.meta.url)))},
            // Bundle the actual page and dev-only import, without firmware/Wasm artifacts.
            external: id => !id.endsWith('/Page.tsx') && !id.endsWith('/dev-share.ts') && id !== './dev-share',
            preserveEntrySignatures: 'strict',
          },
        },
      });
      const chunks = result.output.filter(output => output.type === 'chunk');
      console.log(JSON.stringify({
        code: chunks.map(chunk => chunk.code).join('\\n'),
        modules: chunks.flatMap(chunk => Object.keys(chunk.modules)),
      }));
    `;
    const { stdout } = await promisify(execFile)(
      process.execPath,
      ['--input-type=module', '-e', script],
      {
        env: { ...process.env, NODE_ENV: 'production' },
      },
    );
    const { code, modules } = JSON.parse(stdout);
    const included = mode === 'local-preview';
    expect(modules.some((id) => id.endsWith('/dev-share.ts'))).toBe(included);
    expect(code.includes('/usb-proxy/dev-share')).toBe(included);
    expect(code.includes('Development share registered')).toBe(included);
  });
}
