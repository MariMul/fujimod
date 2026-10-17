"""Guide execution, discovery and the generated clean-image/video contract."""
from io import BytesIO
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from fujimod.walkthroughs import Guide, discover
from fujimod.walkthroughs.media import Video


def png():
    from PIL import Image
    buffer = BytesIO()
    Image.new('RGB', (320, 240), 'white').save(buffer, format='PNG')
    return buffer.getvalue()


class Camera:
    def __init__(self):
        self.png = png()
    def start_recording(self, guide):
        self.guide = guide
        guide.frame(self.png, 0)
    def stop_recording(self):
        pass
    def capture(self):
        return self.png
    def press(self):
        index = self.guide.input('right', True, 0)
        self.guide.frame(self.png, 0.2)
        self.guide.interaction('menu', 'mod/item', input_index=index)
        self.guide.input('right', False, 0.2)


class WalkthroughTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def guide(self):
        camera = Camera()
        return camera, Guide(camera, self.root/'run', identity={'build': 'test'}, title='Test')

    def test_multiple_functions_and_filename_identity(self):
        source = self.root/'example.py'
        source.write_text('from fujimod.walkthroughs import walkthrough\n'
            '@walkthrough("One")\ndef first(camera, guide): pass\n'
            '@walkthrough("Two")\ndef second(camera, guide): pass\n')
        self.assertEqual([(w.name, w.title, w.source) for w in discover(source)],
                         [('first','One',source.resolve()), ('second','Two',source.resolve())])

    @unittest.skipUnless(shutil.which('ffmpeg'), 'ffmpeg required')
    def test_clean_screenshot_and_video_overlay(self):
        camera, guide = self.guide()
        def run(camera, guide):
            with guide.step('Example', 'Description', hold=.2):
                camera.press()
        report = guide.run(run)
        self.assertTrue(report['passed'])
        self.assertEqual((guide.output/'screenshots/001.png').read_bytes(), camera.png)
        self.assertEqual(len(report['interactions']), 1)
        frame = subprocess.check_output(['ffmpeg','-v','error','-i',str(guide.output/'video.mp4'),
                                         '-frames:v','1','-f','image2pipe','-vcodec','png','-'])
        from PIL import Image, ImageStat
        picture = Image.open(BytesIO(frame)).convert('RGB')
        self.assertLess(min(ImageStat.Stat(picture.crop((200,192,308,228))).mean), 240)
        self.assertGreater(min(ImageStat.Stat(picture.crop((0,0,100,100))).mean), 250)
        self.assertEqual(report['steps'][0]['title'], 'Example')
        self.assertIn('video.mp4', report['artifacts'])

    def test_failed_assertion_never_produces_passing_report_or_step(self):
        camera, guide = self.guide()
        def run(camera, guide):
            with guide.step('Will fail'):
                raise AssertionError('wrong setting')
        with self.assertRaisesRegex(AssertionError, 'wrong setting'):
            guide.run(run)
        report = json.loads((guide.output/'guide.json').read_text())
        self.assertFalse(report['passed'])
        self.assertEqual(report['steps'], [])
        self.assertFalse((guide.output/'screenshots').exists())

    def test_fixture_suppresses_coverage(self):
        camera, guide = self.guide()
        def run(camera, guide):
            with guide.fixture('Prepare settings'):
                camera.press()
            with guide.step('Example', hold=0):
                pass
        with patch.object(Video, 'finish'), patch.object(Video, 'still'):
            report = guide.run(run)
        self.assertEqual(report['interactions'], [])
        self.assertTrue(report['fixtures'][0]['passed'])

    def test_nested_steps_refused(self):
        camera, guide = self.guide()
        with guide.step('Outer'):
            with self.assertRaisesRegex(ValueError, 'nest'):
                with guide.step('Inner'):
                    pass
        guide.video.finish()

    def test_invalid_hold(self):
        camera, guide = self.guide()
        for hold in (-1, float('nan'), float('inf'), True, '2'):
            with self.assertRaises(ValueError):
                with guide.step('Invalid', hold=hold):
                    pass

    def test_timestamp_cadence_preserves_short_samples(self):
        camera, guide = self.guide()
        with patch.object(Video, '_write') as write:
            for n in range(101):
                guide.frame(camera.png, n/100)
            self.assertEqual(write.call_count, 15)

    def test_nonmonotonic_video_time_refused(self):
        camera, guide = self.guide()
        guide.frame(camera.png, 1)
        with self.assertRaises(ValueError):
            guide.frame(camera.png, .5)
