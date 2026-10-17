"""Sampled LCD video with timestamped input badges; PNGs stay unannotated."""
from io import BytesIO
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
        self.frames = 0
        self.last_time = None
        self.last_png = None
        self.active = set()
        self.released = {}
        self.size = None
        self.log = None
        self._fractional_frames = 0.0
        self._hold_frames = 0.0
        self._latest_shown = False
        self._finished = False
        self._decoded_png = None
        self._decoded_image = None
        self._font = None

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
            self._fractional_frames += (seconds-self.last_time)*self.fps
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
        # Flush the PREVIOUS interval with its old input state. Sampling need
        # not coincide with edges; an event must never recolor earlier frames.
        self._advance(seconds)
        if pressed:
            self.active.add(name)
            self.released.pop(name, None)
        else:
            self.active.discard(name)
            self.released[name] = self.frames

    def _write(self, png):
        from PIL import Image, ImageDraw, ImageFont
        if png != self._decoded_png:
            self._decoded_image = Image.open(BytesIO(png)).convert('RGBA')
            self._decoded_png = png
        picture = self._decoded_image
        if self.process is None:
            self.size = picture.size
            if any(value % 2 for value in self.size):
                raise ValueError('Video frame dimensions must be even')
            self.log = (self.output/'ffmpeg.log').open('wb')
            try:
                self.process = subprocess.Popen([
                    'ffmpeg', '-v', 'error', '-y', '-f', 'rawvideo', '-pixel_format', 'rgb24',
                    '-video_size', f'{picture.width}x{picture.height}', '-framerate', str(self.fps),
                    '-i', '-', '-an', '-c:v', 'libx264', '-preset', 'fast', '-crf', '20',
                    '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(self.output/'video.mp4'),
                ], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=self.log)
            except BaseException:
                self.log.close()
                raise
        if picture.size != self.size:
            raise ValueError('LCD dimensions changed during walkthrough')
        labels = [(name, 255) for name in sorted(self.active)]
        fade_frames = max(1, round(self.fps*.4))
        for name, released in list(self.released.items()):
            age = self.frames-released
            if age >= fade_frames:
                del self.released[name]
            else:
                labels.append((name, round(255*(1-age/fade_frames))))
        layer = Image.new('RGBA', picture.size)
        draw = ImageDraw.Draw(layer)
        if self._font is None:
            self._font = ImageFont.load_default(size=20)
        font = self._font
        x, y = picture.width-12, picture.height-48
        for name, alpha in reversed(labels):
            label = name.upper().replace('_', ' ')
            while label and draw.textlength(label, font=font)+20 > picture.width-24:
                label = label[:-1]
            width = math.ceil(draw.textlength(label, font=font))+20
            if x-width < 12:
                x, y = picture.width-12, y-42
            draw.rounded_rectangle((x-width, y, x, y+36), radius=6, fill=(0,0,0,round(alpha*.8)))
            draw.text((x-width+10, y+5), label, font=font, fill=(255,255,255,alpha))
            x -= width+6
        self.process.stdin.write(Image.alpha_composite(picture, layer).convert('RGB').tobytes())
        self.frames += 1

    def frame(self, png, seconds):
        self._advance(seconds)
        if png:
            if png != self.last_png:
                self._latest_shown = False
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
                try:
                    self.process.stdin.close()
                except BrokenPipeError:
                    pass
                try:
                    result = self.process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait()
                    raise RuntimeError('Video encoding timed out; see ffmpeg.log')
                finally:
                    self.log.close()
                if result:
                    raise RuntimeError('Video encoding failed; see ffmpeg.log')
