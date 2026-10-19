"""Sampled LCD video with timestamped input badges; PNGs stay unannotated.

Two encodes of the same frames: video.mp4 with the input badges (guides and
previews) and video-clean.mp4 without any helper overlay (website cards,
heroes). Screenshots are always unannotated."""

import math
import shutil
import subprocess
from contextlib import suppress
from io import BytesIO
from pathlib import Path
from typing import BinaryIO, TypedDict

from PIL.Image import Image as PILImage
from PIL.ImageFont import FreeTypeFont, ImageFont

from ..presentation import SPEC, control, draw_badges


class Badge(TypedDict):
    name: str
    released: int | None


def seconds_value(value: float) -> float:
    if (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not math.isfinite(value)
        or value < 0
    ):
        raise ValueError("Time must be a finite nonnegative number of seconds")
    return value


class Video:
    def __init__(self, output: str | Path, *, fps: int = 15) -> None:
        if type(fps) is not int or not 1 <= fps <= 60:
            raise ValueError("fps must be 1..60")
        if not shutil.which("ffmpeg"):
            raise RuntimeError("Walkthrough video requires ffmpeg on PATH")
        self.output, self.fps = Path(output), fps
        self.process: subprocess.Popen[bytes] | None = None
        self.clean: subprocess.Popen[bytes] | None = None
        self.frames = 0
        self.last_time: float | None = None
        self.last_png: bytes | None = None
        self.badges: list[Badge] = []
        self.size: tuple[int, int] | None = None
        self.log: BinaryIO | None = None
        self._fractional_frames = 0.0
        self._hold_frames = 0.0
        self._latest_shown = False
        self._finished = False
        self._decoded_png: bytes | None = None
        self._decoded_image: PILImage | None = None
        self._font: FreeTypeFont | ImageFont | None = None
        self.idle_limit = 0.4
        self.skipped_seconds = 0.0
        self._idle_kept = 0.0

    @property
    def seconds(self) -> float:
        return self.frames / self.fps

    def _advance(self, seconds: float) -> None:
        seconds_value(seconds)
        if self._finished:
            raise RuntimeError("Video is already finished")
        if self.last_time is not None and seconds < self.last_time:
            raise ValueError("Frame and input timestamps must be monotonic")
        if self.last_time is not None and self.last_png:
            elapsed = seconds - self.last_time
            duration = elapsed
            if not self.badges:
                duration = min(duration, max(0, self.idle_limit - self._idle_kept))
                self._idle_kept += duration
            self.skipped_seconds += elapsed - duration
            self._fractional_frames += duration * self.fps
            count = max(0, int(self._fractional_frames + 1e-8))
            self._fractional_frames -= count
            for _ in range(count):
                self._write(self.last_png)
            if count:
                self._latest_shown = True
        self.last_time = seconds

    def input(self, name: str, pressed: bool, seconds: float) -> None:
        if not isinstance(name, str) or not name or type(pressed) is not bool:
            raise ValueError("Input needs a name and boolean pressed state")
        control(
            name
        )  # Reject unconfigured IDs instead of displaying raw emulator names.
        # Flush the PREVIOUS interval with its old input state. Sampling need
        # not coincide with edges; an event must never recolor earlier frames.
        self._advance(seconds)
        self._idle_kept = 0
        if pressed:
            self.badges.append(dict(name=name, released=None))
            self.badges = self.badges[-5:]
        else:
            for badge in reversed(self.badges):
                if badge["name"] == name and badge["released"] is None:
                    badge["released"] = self.frames
                    break

    def _labels(self) -> list[tuple[str, int]]:
        lifetime_frames = 5 * self.fps
        fade_frames = max(1, round(self.fps * SPEC["badge"]["release_ms"] / 1000))
        self.badges = [
            badge
            for badge in self.badges
            if badge["released"] is None
            or self.frames - badge["released"] < lifetime_frames
        ]
        return [
            (
                badge["name"],
                255
                if badge["released"] is None
                else round(
                    255
                    * min(
                        1,
                        (lifetime_frames - (self.frames - badge["released"]))
                        / fade_frames,
                    )
                ),
            )
            for badge in self.badges
        ]

    def _write(self, png: bytes) -> None:
        from PIL import Image, ImageOps

        if png != self._decoded_png:
            self._decoded_image = Image.open(BytesIO(png)).convert("RGBA")
            if self.size is not None and self._decoded_image.size != self.size:
                self._decoded_image = ImageOps.pad(
                    self._decoded_image, self.size, color=(0, 0, 0, 255)
                )
            self._decoded_png = png
        picture = self._decoded_image
        assert picture is not None
        if self.process is None:
            self.size = picture.size
            if any(value % 2 for value in self.size):
                raise ValueError("Video frame dimensions must be even")
            self.log = (self.output / "ffmpeg.log").open("wb")

            def encoder(name: str) -> subprocess.Popen[bytes]:
                return subprocess.Popen(
                    [
                        "ffmpeg",
                        "-v",
                        "error",
                        "-y",
                        "-f",
                        "rawvideo",
                        "-pixel_format",
                        "rgb24",
                        "-video_size",
                        f"{picture.width}x{picture.height}",
                        "-framerate",
                        str(self.fps),
                        "-i",
                        "-",
                        "-an",
                        "-c:v",
                        "libx264",
                        "-preset",
                        "fast",
                        "-crf",
                        "20",
                        "-pix_fmt",
                        "yuv420p",
                        "-movflags",
                        "+faststart",
                        str(self.output / name),
                    ],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.DEVNULL,
                    stderr=self.log,
                )

            try:
                self.process = encoder("video.mp4")
                self.clean = encoder("video-clean.mp4")
            except BaseException:
                if self.process is not None:
                    self.process.kill()
                self.log.close()
                raise
        if picture.size != self.size:
            raise ValueError("LCD dimensions changed during walkthrough")
        assert self.process is not None and self.process.stdin is not None
        assert self.clean is not None and self.clean.stdin is not None
        self.process.stdin.write(
            draw_badges(picture, self._labels(), align="right", layout="stack")
            .convert("RGB")
            .tobytes()
        )
        self.clean.stdin.write(picture.convert("RGB").tobytes())
        self.frames += 1

    def frame(self, png: bytes, seconds: float) -> None:
        self._advance(seconds)
        if png:
            if png != self.last_png:
                self._latest_shown = False
                self._idle_kept = 0
            self.last_png = png

    def still(self, png: bytes, seconds: float) -> None:
        seconds_value(seconds)
        if self._finished:
            raise RuntimeError("Video is already finished")
        if png != self.last_png:
            self._latest_shown = False
        self.last_png = png
        self._hold_frames += seconds * self.fps
        count = max(0, int(self._hold_frames + 1e-8))
        self._hold_frames -= count
        for _ in range(count):
            self._write(png)
        if count:
            self._latest_shown = True
            self._idle_kept = self.idle_limit

    def finish(self) -> None:
        if self._finished:
            return
        self._finished = True
        try:
            # The newest observation must appear even without a later sample
            # or a guide hold. Previously it disappeared at end of recording.
            if self.last_png and not self._latest_shown:
                self._write(self.last_png)
            if self.process is None:
                raise RuntimeError("No LCD frames recorded")
        finally:
            if self.process is not None:
                results = []
                try:
                    for process in (self.process, self.clean):
                        assert process is not None and process.stdin is not None
                        with suppress(BrokenPipeError):
                            process.stdin.close()
                    for process in (self.process, self.clean):
                        assert process is not None
                        try:
                            results.append(process.wait(timeout=30))
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait()
                            raise RuntimeError(
                                "Video encoding timed out; see ffmpeg.log"
                            ) from None
                finally:
                    assert self.log is not None
                    self.log.close()
                if any(results):
                    raise RuntimeError("Video encoding failed; see ffmpeg.log")
