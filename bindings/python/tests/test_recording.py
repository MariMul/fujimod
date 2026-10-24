"""The shared native source protocol and screenshot-default recording API."""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from fujimod.recording import Frame, Recorder, capture_scanout


class Source:
    def __init__(self) -> None:
        self.calls = 0
        self.instructions, self.ticks = 123, 456

    def recording_frame(
        self, node: str, instance: int, diagnostics: bool
    ) -> tuple[bytes, str, int, int]:
        assert node == "scanout" and instance == 0
        self.calls += 1
        return (
            bytes([40]) * 320 * 240 * 3,
            json.dumps(dict(instance=instance)),
            320,
            240,
        )


class RecordingTests(unittest.TestCase):
    def test_pixels_preserve_the_unencoded_rgb_raster(self) -> None:
        pixels = bytes((10, 20, 30, 40, 50, 60))
        frame = Frame.rgb(2, 1, pixels)
        self.assertEqual(frame.pixels(), pixels)
        self.assertEqual(Frame.from_png(frame.png()).pixels(), pixels)

    def test_screenshot_default_samples_just_the_start_and_never_requires_encoders(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            recorder, source = Recorder(root), Source()
            recorder.sample_scanout(source, 0)
            for index in range(100):
                recorder.sample_scanout(source, index / 15, force=True)
            frame = capture_scanout(source)
            assert frame is not None
            digest = recorder.screenshot(frame, "screenshots/001.png")
            recorder.still(frame, 1.5)
            recorder.finish()
            self.assertEqual(source.calls, 2)
            self.assertEqual((source.instructions, source.ticks), (123, 456))
            self.assertEqual(
                digest,
                hashlib.sha256((root / "screenshots/001.png").read_bytes()).hexdigest(),
            )
            self.assertEqual(frame.sha256, digest)
            self.assertEqual(json.loads(recorder.metrics())["written_frames"], 0)
            self.assertFalse((root / "video.mp4").exists())
            self.assertFalse((root / "ffmpeg.log").exists())

    def test_badges_do_not_change_clean_frames_or_native_resolution(self) -> None:
        frame = Frame.rgb(320, 240, bytes([40]) * 320 * 240 * 3)
        before = frame.png()
        annotated = frame.annotated([("right", 255)])
        self.assertEqual(frame.png(), before)
        self.assertNotEqual(annotated.png(), before)
        self.assertEqual((annotated.width, annotated.height), (320, 240))
        with self.assertRaisesRegex(ValueError, "Missing input presentation"):
            frame.annotated([("unconfigured control", 255)])
