import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { afterEach, beforeEach, expect, test, vi } from 'vite-plus/test';
import { FeatureTile } from './FeatureTile';

let dispose: (() => void) | undefined;
const play = vi.fn<() => Promise<void>>();
const pause = vi.fn<() => void>();
beforeEach(() => {
  play.mockReset().mockResolvedValue();
  pause.mockReset();
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(play);
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(pause);
});
afterEach(() => {
  dispose?.();
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

function mount() {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => (
      <FeatureTile media={{ still: 'hero.webp', video: 'hero.mp4' }}>
        <h4>Feature</h4>
        <button>Option</button>
      </FeatureTile>
    ),
    host,
  );
  flush();
  const tile = host.querySelector('article')!;
  const video = host.querySelector('video')!;
  const fire = (type: string) => {
    tile.dispatchEvent(new MouseEvent(type));
    flush();
  };
  return { host, tile, video, fire };
}

test('stays on the screenshot until playback starts and resets on mouse leave', () => {
  const { host, video, fire } = mount();
  expect(host.querySelector('img')?.getAttribute('src')).toBe('/artifacts/docs/hero.webp');
  expect(video.hasAttribute('src')).toBe(false);
  expect(video.autoplay).toBe(false);
  expect(play).not.toHaveBeenCalled();
  fire('mouseenter');
  expect(video.getAttribute('src')).toBe('/artifacts/docs/hero.mp4');
  expect(play).toHaveBeenCalledOnce();
  expect(video.classList.contains('playing')).toBe(false);
  video.dispatchEvent(new Event('playing'));
  flush();
  expect(video.classList.contains('playing')).toBe(true);
  video.currentTime = 3;
  fire('mouseleave');
  expect(pause).toHaveBeenCalledOnce();
  expect(video.currentTime).toBe(0);
  expect(video.classList.contains('playing')).toBe(false);
  // A late playback event after leaving must never replace the screenshot.
  video.dispatchEvent(new Event('playing'));
  flush();
  expect(video.classList.contains('playing')).toBe(false);
  fire('mouseenter');
  expect(play).toHaveBeenCalledTimes(2);
});

test('keeps playing while focus moves inside the tile or the pointer remains over it', () => {
  const { tile, host, fire } = mount();
  tile.focus();
  flush();
  expect(play).toHaveBeenCalledOnce();
  host.querySelector('button')!.focus();
  flush();
  expect(pause).not.toHaveBeenCalled();
  fire('mouseenter');
  host.querySelector('button')!.blur();
  flush();
  expect(pause).not.toHaveBeenCalled();
  fire('mouseleave');
  expect(pause).toHaveBeenCalledOnce();
});

test('keeps the screenshot when the video is missing or playback is rejected', async () => {
  const { host, video, fire } = mount();
  play.mockRejectedValue(new Error('Playback blocked'));
  fire('mouseenter');
  await Promise.resolve();
  flush();
  expect(video.classList.contains('playing')).toBe(false);
  video.dispatchEvent(new Event('error'));
  flush();
  fire('mouseleave');
  fire('mouseenter');
  expect(play).toHaveBeenCalledOnce();
  expect(host.querySelector('img')).not.toBeNull();
});
