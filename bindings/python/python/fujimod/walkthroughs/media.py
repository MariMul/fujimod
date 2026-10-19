"""Sampled LCD video with timestamped input badges; PNGs stay unannotated.

Two encodes of the same frames: video.mp4 with the input badges (guides and
previews) and video-clean.mp4 without any helper overlay (website cards,
heroes). Screenshots are always unannotated."""
from io import BytesIO
from ..presentation import SPEC, control, draw_badges
import math
from pathlib import Path
import shutil
import subprocess


def seconds_value(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
        raise ValueError('Time must be a finite nonnegative number of seconds')
    return value


class Video:
    def __init__(self, output, *, fps=15):
        if type(fps) is not int or not 1 <= fps <= 60:
            raise ValueError('fps must be 1..60')
        if not shutil.which('ffmpeg'):
            raise RuntimeError('Walkthrough video requires ffmpeg on PATH')
        self.output, self.fps = Path(output), fps
        self.process = None
        self.clean = None
        self.frames = 0
        self.last_time = None
        self.last_png = None
        self.badges = []
        self.size = None
        self.log = None
        self._fractional_frames = 0.0
        self._hold_frames = 0.0
        self._latest_shown = False
        self._finished = False
        self._decoded_png = None
        self._decoded_image = None
        self._font = None
        self.idle_limit = .4
        self.skipped_seconds = 0.0
        self._idle_kept = 0.0

    @property
    def seconds(self):
        return self.frames / self.fps

    def _advance(self, seconds):
        seconds_value(seconds)
        if self._finished:
            raise RuntimeError('Video is already finished')
        if self.last_time is not None and seconds < self.last_time:
            raise ValueError('Frame and input timestamps must be monotonic')
        if self.last_time is not None and self.last_png:
            elapsed = seconds-self.last_time
            duration = elapsed
            if not self.badges:
                duration = min(duration, max(0, self.idle_limit-self._idle_kept))
                self._idle_kept += duration
            self.skipped_seconds += elapsed-duration
            self._fractional_frames += duration*self.fps
            count = max(0, int(self._fractional_frames+1e-8))
            self._fractional_frames -= count
            for _ in range(count):
                self._write(self.last_png)
            if count:
                self._latest_shown = True
        self.last_time = seconds

    def input(self, name, pressed, seconds):
        if not isinstance(name, str) or not name or type(pressed) is not bool:
            raise ValueError('Input needs a name and boolean pressed state')
        control(name)  # Reject unconfigured IDs instead of displaying raw emulator names.
        # Flush the PREVIOUS interval with its old input state. Sampling need
        # not coincide with edges; an event must never recolor earlier frames.
        self._advance(seconds)
        self._idle_kept = 0
        if pressed:
            self.badges.append(dict(name=name, released=None))
            self.badges = self.badges[-5:]
        else:
            for badge in reversed(self.badges):
                if badge['name'] == name and badge['released'] is None:
                    badge['released'] = self.frames
                    break

    def _labels(self):
        lifetime_frames = 5*self.fps
        fade_frames = max(1, round(self.fps*SPEC['badge']['release_ms']/1000))
        self.badges = [badge for badge in self.badges if badge['released'] is None
                       or self.frames-badge['released'] < lifetime_frames]
        return [(badge['name'], 255 if badge['released'] is None else
                 round(255*min(1, (lifetime_frames-(self.frames-badge['released']))/fade_frames)))
                for badge in self.badges]

    def _write(self, png):
        from PIL import Image, ImageOps
        if png != self._decoded_png:
            self._decoded_image = Image.open(BytesIO(png)).convert('RGBA')
            if self.size is not None and self._decoded_image.size != self.size:
                self._decoded_image = ImageOps.pad(self._decoded_image, self.size, color=(0, 0, 0, 255))
            self._decoded_png = png
        picture = self._decoded_image
        if self.process is None:
            self.size = picture.size
            if any(value % 2 for value in self.size):
                raise ValueError('Video frame dimensions must be even')
            self.log = (self.output/'ffmpeg.log').open('wb')
            def encoder(name):
                return subprocess.Popen([
                    'ffmpeg', '-v', 'error', '-y', '-f', 'rawvideo', '-pixel_format', 'rgb24',
                    '-video_size', f'{picture.width}x{picture.height}', '-framerate', str(self.fps),
                    '-i', '-', '-an', '-c:v', 'libx264', '-preset', 'fast', '-crf', '20',
                    '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(self.output/name),
                ], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=self.log)
            try:
                self.process = encoder('video.mp4')
                self.clean = encoder('video-clean.mp4')
            except BaseException:
                if self.process is not None:
                    self.process.kill()
                self.log.close()
                raise
        if picture.size != self.size:
            raise ValueError('LCD dimensions changed during walkthrough')
        self.process.stdin.write(draw_badges(picture, self._labels(), align='right',
                                            layout='stack').convert('RGB').tobytes())
        self.clean.stdin.write(picture.convert('RGB').tobytes())
        self.frames += 1

    def frame(self, png, seconds):
        self._advance(seconds)
        if png:
            if png != self.last_png:
                self._latest_shown = False
                self._idle_kept = 0
            self.last_png = png

    def still(self, png, seconds):
        seconds_value(seconds)
        if self._finished:
            raise RuntimeError('Video is already finished')
        if png != self.last_png:
            self._latest_shown = False
        self.last_png = png
        self._hold_frames += seconds*self.fps
        count = max(0, int(self._hold_frames+1e-8))
        self._hold_frames -= count
        for _ in range(count):
            self._write(png)
        if count:
            self._latest_shown = True
            self._idle_kept = self.idle_limit

    def finish(self):
        if self._finished:
            return
        self._finished = True
        try:
            # The newest observation must appear even without a later sample
            # or a guide hold. Previously it disappeared at end of recording.
            if self.last_png and not self._latest_shown:
                self._write(self.last_png)
            if self.process is None:
                raise RuntimeError('No LCD frames recorded')
        finally:
            if self.process is not None:
                results = []
                try:
                    for process in (self.process, self.clean):
                        try:
                            process.stdin.close()
                        except BrokenPipeError:
                            pass
                    for process in (self.process, self.clean):
                        try:
                            results.append(process.wait(timeout=30))
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait()
                            raise RuntimeError('Video encoding timed out; see ffmpeg.log')
                finally:
                    self.log.close()
                if any(results):
                    raise RuntimeError('Video encoding failed; see ffmpeg.log')
