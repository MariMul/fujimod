import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { rendererBackend, rendererURL } from './renderer';

test('renderer selection is explicit and switching preserves the scenario, environment and fragment', () => {
  assert.equal(rendererBackend(''), 'webgl2');
  assert.equal(rendererBackend('?renderer=webgpu'), 'webgpu');
  assert.equal(rendererBackend('?renderer=webgl2'), 'webgl2');
  assert.throws(() => rendererBackend('?renderer=unknown'), /Unknown 3D renderer/);
  const before = 'https://example.test/?environment=station&scenario=record#viewer';
  const after = new URL(rendererURL(before, 'webgpu'));
  assert.equal(after.searchParams.get('environment'), 'station');
  assert.equal(after.searchParams.get('scenario'), 'record');
  assert.equal(after.hash, '#viewer');
  assert.equal(rendererBackend(after.search), 'webgpu');
  assert.equal(rendererBackend(new URL(rendererURL(after.href, 'webgl2')).search), 'webgl2');
});
