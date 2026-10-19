import { Show, createSignal, onSettled } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { fadeLoop } from './fade-loop';
import { watchFeaturePreview } from './feature-preview';

/** Mounted only while this tile owns the catalog's single playback slot. */
function PreviewVideo(props: { src: string; failed: () => void }) {
  let video: HTMLVideoElement | undefined;
  let disposed = false;
  const [playing, setPlaying] = createSignal(false);
  const failed = () => {
    if (!disposed) props.failed();
  };
  onSettled(() => {
    const element = video!;
    // Set the property as well as the attribute for mobile autoplay policies.
    element.muted = true;
    const loop = fadeLoop(element);
    void element.play().catch(failed);
    return () => {
      disposed = true;
      loop.dispose();
      element.pause();
      element.removeAttribute('src');
      element.load();
    };
  });
  return (
    <video
      ref={(element) => {
        video = element;
      }}
      class={['catalog-tile-video', { playing: playing() }]}
      src={props.src}
      muted
      playsinline
      preload="none"
      aria-hidden="true"
      onPlaying={() => {
        if (!disposed) setPlaying(true);
      }}
      onError={failed}
    />
  );
}

/** Screenshots at rest; viewport playback on touch devices, hover/focus on desktop. */
export function FeatureTile(props: {
  media?: { still: string; video: string } | null;
  children: JSX.Element;
}) {
  let preview: HTMLDivElement | undefined;
  let playback: ReturnType<typeof watchFeaturePreview> | undefined;
  const [active, setActive] = createSignal(false);
  onSettled(() => {
    if (!preview) return;
    playback = watchFeaturePreview(preview, setActive);
    return () => playback?.dispose();
  });

  return (
    <article
      class="catalog-tile"
      tabindex="0"
      onMouseEnter={() => playback?.hover(true)}
      onMouseLeave={() => playback?.hover(false)}
      onFocusIn={() => playback?.focus(true)}
      onFocusOut={(event) => {
        if (
          event.relatedTarget instanceof Node &&
          event.currentTarget.contains(event.relatedTarget)
        )
          return;
        playback?.focus(false);
      }}
    >
      <Show when={props.media}>
        {(media) => (
          <div
            class="catalog-tile-preview"
            ref={(element) => {
              preview = element;
            }}
          >
            <img src={`/artifacts/docs/${media().still}`} alt="" loading="lazy" />
            <Show when={active()}>
              <PreviewVideo
                src={`/artifacts/docs/${media().video}`}
                failed={() => playback?.disable()}
              />
            </Show>
          </div>
        )}
      </Show>
      {props.children}
    </article>
  );
}
