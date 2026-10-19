import { test } from 'vite-plus/test';
import assert from 'node:assert/strict';
import { featureMediaFiles } from '../scripts/stage-site.ts';
import { featureMedia, validateFeatureMedia } from '../src/home/feature-media.ts';

const catalog = 'a'.repeat(24);
const manifest = validateFeatureMedia({
  schema: 1,
  catalog,
  cards: {
    shape_aspect_item: {
      video: '1'.repeat(24) + '.mp4',
      still: '2'.repeat(24) + '.webp',
      clips: { '65:24': [1, 2.5] },
    },
  },
});

test('media rendered for other catalog text is never shown', () => {
  assert.equal(
    featureMedia(catalog, manifest)('missing', 'shape_aspect_item')?.still,
    '2'.repeat(24) + '.webp',
  );
  assert.equal(featureMedia('b'.repeat(24), manifest)('shape_aspect_item'), null);
  assert.equal(featureMedia(catalog, null)('shape_aspect_item'), null);
});

test('deployment stages media only for the catalog being deployed', () => {
  const source = `const media = featureMedia('${catalog}');\nmedia('shape_aspect_item')`;
  assert.deepEqual(
    [...featureMediaFiles(manifest, source)],
    ['1'.repeat(24) + '.mp4', '2'.repeat(24) + '.webp'],
  );
  assert.throws(
    () => featureMediaFiles(manifest, source.replace(catalog, 'b'.repeat(24))),
    /rendered for catalog/,
  );
  assert.throws(
    () => featureMediaFiles(manifest, `const media = featureMedia('${catalog}');`),
    /Unknown feature media card/,
  );
  assert.throws(
    () =>
      validateFeatureMedia({
        ...manifest,
        cards: { x: { video: '../x.mp4', still: '', clips: {} } },
      }),
    /Invalid feature media: x/,
  );
});
