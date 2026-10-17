"""Presentation pauses belong at maneuver boundaries, outside input timing."""
from contextlib import contextmanager
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import Mock

from fujimod.walkthroughs import Guide, open_menu


class ManeuverPacingTests(TestCase):
    def test_illustrated_destination_holds_for_one_and_a_half_seconds(self):
        with TemporaryDirectory() as directory:
            guide = Guide.__new__(Guide)
            guide.output = Path(directory)
            guide.camera = SimpleNamespace(capture=Mock(return_value=b'settled image'))
            guide.video = SimpleNamespace(seconds=0, still=Mock())
            guide.report = dict(steps=[])
            guide._inside_step = guide._preparing = False
            guide._failures = []
            guide.save = Mock()
            with guide.step('Choose the tile'):
                pass
            guide.video.still.assert_called_once_with(b'settled image', 1.5)
            self.assertEqual(guide.report['steps'][0]['hold'], 1.5)

    def test_menu_starter_pauses_after_activation_before_following_navigation(self):
        events = []
        @contextmanager
        def step(title):
            events.append(('instruction', title))
            yield
            events.append(('screenshot', 'highlighted item'))
        camera = SimpleNamespace(
            menu_path=lambda item: 'MENU > ITEM',
            select_menu_item=lambda item: events.append(('select', item)),
            open_menu_item=lambda item: events.append(('open', item)))
        guide = SimpleNamespace(step=step, pause=lambda seconds: events.append(('hold', seconds)))
        open_menu('item', title='Item settings')(camera, guide)
        self.assertEqual(events, [
            ('instruction', 'Choose MENU > ITEM'), ('select', 'item'),
            ('screenshot', 'highlighted item'), ('open', 'item'), ('hold', 1.5)])
