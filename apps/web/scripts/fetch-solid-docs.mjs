import { mkdir, writeFile } from 'node:fs/promises';

// Generated reference material stays out of the application's source and assets.
const output = new URL('../build/docs/solidjs/', import.meta.url);
const pages = [
  'llms.txt',
  'concepts/intro-to-reactivity.md',
  'concepts/signals.md',
  'concepts/effects.md',
  'concepts/stores.md',
  'concepts/components/basics.md',
  'configuration/typescript.md',
  'guides/testing.md',
  'reference/lifecycle/on-cleanup.md',
];
for (const page of pages) {
  const url = new URL(page, 'https://docs.solidjs.com/');
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  const file = new URL(page, output);
  await mkdir(new URL('.', file), { recursive: true });
  await writeFile(file, await response.text());
}
console.log(`Official SolidJS docs downloaded to ${output.pathname}`);
