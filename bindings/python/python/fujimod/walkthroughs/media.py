"""Sampled LCD video with input badges; source PNGs are never annotated."""
from io import BytesIO
import math
from pathlib import Path
import shutil
import subprocess


class Video:
    def __init__(self, output, *, fps=15):
        if not isinstance(fps, int) or not 1 <= fps <= 60:
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

    @property
    def seconds(self):
        return self.frames / self.fps

    def input(self, name, pressed):
        if pressed:
            self.active.add(name)
            self.released.pop(name, None)
        else:
            self.active.discard(name)
            self.released[name] = self.frames

    def _write(self, png):
        from PIL import Image, ImageDraw, ImageFont
        picture = Image.open(BytesIO(png)).convert('RGBA')
        if self.process is None:
            self.size = picture.size
            if any(value % 2 for value in self.size):
                raise ValueError('Video frame dimensions must be even')
            self.log = (self.output / 'ffmpeg.log').open('wb')
            self.process = subprocess.Popen([
                'ffmpeg', '-v', 'error', '-y', '-f', 'rawvideo', '-pixel_format', 'rgb24',
                '-video_size', f'{picture.width}x{picture.height}', '-framerate', str(self.fps),
                '-i', '-', '-an', '-c:v', 'libx264', '-preset', 'fast', '-crf', '20',
                '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(self.output / 'video.mp4'),
            ], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=self.log)
        if picture.size != self.size:
            raise ValueError('LCD dimensions changed during walkthrough')
        labels = [(name, 255) for name in sorted(self.active)]
        fade_frames = max(1, round(self.fps * 0.4))
        for name, released in list(self.released.items()):
            age = self.frames - released
            if age >= fade_frames:
                del self.released[name]
            else:
                labels.append((name, round(255 * (1 - age / fade_frames))))
        layer = Image.new('RGBA', picture.size)
        draw = ImageDraw.Draw(layer)
        font = ImageFont.load_default(size=20)
        x = picture.width - 12
        for name, alpha in reversed(labels):
            label = name.upper().replace('_', ' ')
            width = math.ceil(draw.textlength(label, font=font)) + 20
            draw.rounded_rectangle((x-width, picture.height-48, x, picture.height-12),
                                   radius=6, fill=(0, 0, 0, round(alpha * .8)))
            draw.text((x-width+10, picture.height-43), label, font=font, fill=(255,255,255,alpha))
            x -= width + 6
        picture = Image.alpha_composite(picture, layer).convert('RGB')
        self.process.stdin.write(picture.tobytes())
        self.frames += 1

    def frame(self, png, seconds):
        if not png:
            return
        if not math.isfinite(seconds) or (self.last_time is not None and seconds < self.last_time):
            raise ValueError('Frame timestamps must be finite and monotonic')
        # Fill the elapsed interval with its previous observation. New pixels
        # are never shown before their observation time. Sampling is explicit.
        if self.last_time is not None:
            self._fractional_frames += (seconds - self.last_time) * self.fps
            count = max(0, int(self._fractional_frames + 1e-8))
            self._fractional_frames -= count
            for _ in range(count):
                self._write(self.last_png)
        self.last_time, self.last_png = seconds, png

    def still(self, png, seconds):
        self.last_png = png
        for _ in range(max(1, round(seconds * self.fps))):
            self._write(png)

    def finish(self):
        if self.process is None and self.last_png:
            self._write(self.last_png)
        if self.process is None:
            raise RuntimeError('No LCD frames recorded')
        try:
            self.process.stdin.close()
        finally:
            result = self.process.wait()
            self.log.close()
        if result:
            raise RuntimeError('Video encoding failed; see ffmpeg.log')
