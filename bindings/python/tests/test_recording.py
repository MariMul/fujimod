"""The shared native source protocol and screenshot-default recording API."""

import hashlib
import json
import shutil
import subprocess
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
    @unittest.skipUnless(shutil.which('ffmpeg'), 'ffmpeg required')
    def test_sd_panel_stays_in_video_while_native_screenshot_keeps_its_pixels(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            recorder = Recorder(root, fps=10, video=True)
            frame = Frame.rgb(720, 480, bytes((40, 80, 120)) * 720 * 480)
            recorder.reserve_card_panel()
            recorder.frame(frame, 0)
            recorder.still(frame, .2)
            panel = dict(label='2026-10-25', rows=[dict(name='DCIM', path='/DCIM',
                depth=0, directory=True, bytes=0), dict(name='DSCF0001.JPG',
                path='/DCIM/DSCF0001.JPG', depth=1, directory=False, bytes=123456)],
                highlight='/DCIM/DSCF0001.JPG')
            recorder.card_panel(json.dumps(panel))
            recorder.still(frame, .4)
            recorder.screenshot(frame, 'native.png')
            with self.assertRaisesRegex(ValueError, 'before presenting'):
                recorder.reserve_card_panel()
            recorder.finish()
            self.assertEqual(Frame.from_png((root/'native.png').read_bytes()).pixels(), frame.pixels())
            from PIL import Image
            for movie in ('video.mp4', 'video-clean.mp4'):
                raw = subprocess.check_output(['ffmpeg', '-v', 'error', '-i', str(root/movie),
                    '-f', 'rawvideo', '-pix_fmt', 'rgb24', 'pipe:1'])
                self.assertEqual(len(raw), 6 * 1080 * 480 * 3)
                first = Image.frombytes('RGB', (1080, 480), raw[:1080*480*3])
                last = Image.frombytes('RGB', (1080, 480), raw[-1080*480*3:])
                self.assertEqual(first.getpixel((500, 200)), last.getpixel((500, 200)))
                self.assertNotEqual(first.getpixel((760, 150)), last.getpixel((760, 150)))

    def test_pixels_preserve_the_unencoded_rgb_raster(self) -> None:
        pixels = bytes((10, 20, 30, 40, 50, 60))
        frame = Frame.rgb(2, 1, pixels)
        self.assertEqual(frame.pixels(), pixels)
        self.assertEqual(Frame.from_png(frame.png()).pixels(), pixels)

    def test_idle_limit_can_preserve_a_required_observation_interval(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            recorder=Recorder(Path(directory),fps=10)
            frame=Frame.rgb(2,1,bytes((10,20,30))*2)
            recorder.frame(frame,0)
            recorder.idle_limit=2.9
            recorder.frame(frame,2.5)
            self.assertEqual(recorder.frames,25)
            self.assertEqual(recorder.skipped_seconds,0)
            for value in (-1,float('nan'),float('inf')):
                with self.assertRaises(ValueError):recorder.idle_limit=value
            self.assertEqual(recorder.idle_limit,2.9)
            recorder.finish()

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
        annotated = frame.annotated([("selector.right", 255)])
        self.assertEqual(frame.png(), before)
        self.assertNotEqual(annotated.png(), before)
        self.assertEqual((annotated.width, annotated.height), (320, 240))
        with self.assertRaisesRegex(ValueError, "Missing input presentation"):
            frame.annotated([("unconfigured control", 255)])
