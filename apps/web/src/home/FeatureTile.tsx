import { Show, createSignal, onCleanup } from 'solid-js';
import type { JSX } from '@solidjs/web';

/** A screenshot that gives way to its recording while the tile is hovered or focused. */
export function FeatureTile(props: {
  media?: { still: string; video: string } | null;
  children: JSX.Element;
}) {
  let video: HTMLVideoElement | undefined;
  let hovered = false;
  let focused = false;
  let failed = false;
  const [playing, setPlaying] = createSignal(false);

  const update = () => {
    if (!video) return;
    if ((hovered || focused) && !failed) {
      if (!video.getAttribute('src')) video.src = `/artifacts/docs/${props.media!.video}`;
      void video.play().catch(() => setPlaying(false));
    } else {
      video.pause();
      video.currentTime = 0;
      setPlaying(false);
    }
  };
  onCleanup(() => video?.pause());

  return (
    <article
      class="catalog-tile"
      tabindex="0"
      onMouseEnter={() => {
        hovered = true;
        update();
      }}
      onMouseLeave={() => {
        hovered = false;
        update();
      }}
      onFocusIn={() => {
        focused = true;
        update();
      }}
      onFocusOut={(event) => {
        if (
          event.relatedTarget instanceof Node &&
          event.currentTarget.contains(event.relatedTarget)
        )
          return;
        focused = false;
        update();
      }}
    >
      <Show when={props.media}>
        {(media) => (
          <div class="catalog-tile-preview">
            <img src={`/artifacts/docs/${media().still}`} alt="" loading="lazy" />
            <video
              ref={(element) => {
                video = element;
              }}
              class={['catalog-tile-video', { playing: playing() }]}
              muted
              loop
              playsinline
              preload="none"
              aria-hidden="true"
              onPlaying={() => {
                if (hovered || focused) setPlaying(true);
                else video?.pause();
              }}
              onError={() => {
                failed = true;
                update();
              }}
            />
          </div>
        )}
      </Show>
      {props.children}
    </article>
  );
}
