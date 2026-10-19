/** Preview recording: clean clip, still, and each option's range in the clip. */
export type Media = { video: string; still: string; clips: Record<string, [number, number]> };

/** artifacts/docs/media.json: recordings rendered for one catalog text, by card. */
export type FeatureMedia = { schema: 1; catalog: string; cards: Record<string, Media> };

declare const __FUJIMOD_FEATURE_MEDIA__: FeatureMedia | null;

export function validateFeatureMedia(value: unknown): FeatureMedia {
  const manifest = value as FeatureMedia;
  if (
    manifest?.schema !== 1 ||
    !/^[0-9a-f]{24}$/.test(manifest.catalog) ||
    typeof manifest.cards !== 'object'
  )
    throw Error('Invalid feature media manifest');
  for (const [card, media] of Object.entries(manifest.cards))
    if (
      !/^[0-9a-f]{24}\.mp4$/.test(media.video) ||
      !/^[0-9a-f]{24}\.webp$/.test(media.still) ||
      Object.values(media.clips).some(
        (range) => range.length !== 2 || !(range[0] >= 0 && range[1] > range[0]),
      )
    )
      throw Error(`Invalid feature media: ${card}`);
  return manifest;
}

/** Media lookup for the catalog with this digest; media rendered for other text is never shown. */
export function featureMedia(catalog: string, manifest = __FUJIMOD_FEATURE_MEDIA__) {
  const cards = manifest?.catalog === catalog ? manifest.cards : {};
  /** The first of these cards that has media. */
  return (...preference: string[]): Media | null =>
    preference.map((card) => cards[card]).find(Boolean) ?? null;
}
