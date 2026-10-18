"""Guide execution, discovery and the generated clean-image/video contract."""

import json
import shutil
import subprocess
import tempfile
import unittest
from collections.abc import Callable
from io import BytesIO
from pathlib import Path
from unittest.mock import patch

from fujimod.walkthroughs import Guide, discover
from fujimod.walkthroughs.media import Video
from fujimod.walkthroughs.text import MenuEntry, MenuReference, plain


def png(color: str = "white") -> bytes:
    from PIL import Image

    buffer = BytesIO()
    Image.new("RGB", (320, 240), color).save(buffer, format="PNG")
    return buffer.getvalue()


class Camera:
    guide: Guide[Camera]
    value: int = 0
    menu_reference: Callable[[MenuReference], MenuEntry]
    select_menu_item: Callable[[MenuReference], None]
    open_menu_item: Callable[[MenuReference], None]

    def __init__(self) -> None:
        self.png = png()

    def start_recording(self, guide: Guide[Camera]) -> None:
        self.guide = guide
        guide.frame(self.png, 0)

    def stop_recording(self) -> None:
        pass

    def begin_segment(self) -> None:
        self.guide.frame(self.png, 0)

    def capture(self) -> bytes:
        return self.png

    def press(self) -> None:
        index = self.guide.input("right", True, 0)
        self.guide.frame(self.png, 0.2)
        self.guide.interaction("menu", "mod/item", input_index=index)
        self.guide.input("right", False, 0.2)


class WalkthroughTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def guide(self) -> tuple[Camera, Guide[Camera]]:
        camera = Camera()
        return camera, Guide(
            camera, self.root / "run", identity={"build": "test"}, title="Test"
        )

    def test_multiple_functions_and_filename_identity(self) -> None:
        source = self.root / "example.py"
        source.write_text(
            "from fujimod.walkthroughs import walkthrough\n"
            '@walkthrough("One")\ndef first(camera, guide): pass\n'
            '@walkthrough("Two")\ndef second(camera, guide): pass\n'
        )
        self.assertEqual(
            [(w.name, w.title, w.source) for w in discover(source)],
            [("first", "One", source.resolve()), ("second", "Two", source.resolve())],
        )

    @unittest.skipUnless(shutil.which("ffmpeg"), "ffmpeg required")
    def test_clean_screenshot_and_video_overlay(self) -> None:
        camera, guide = self.guide()

        def run(camera: Camera, guide: Guide[Camera]) -> None:
            with guide.step("Example", "Description", hold=0.2):
                camera.press()

        report = guide.run(run)
        self.assertTrue(report["passed"])
        self.assertEqual(
            (guide.output / "screenshots/001.png").read_bytes(), camera.png
        )
        self.assertEqual(len(report["interactions"]), 1)
        frame = subprocess.check_output([
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(guide.output / "video.mp4"),
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "-",
        ])
        from PIL import Image, ImageStat

        picture = Image.open(BytesIO(frame)).convert("RGB")
        self.assertLess(
            min(ImageStat.Stat(picture.crop((260, 140, 308, 178))).mean), 240
        )
        self.assertGreater(
            min(ImageStat.Stat(picture.crop((0, 0, 100, 100))).mean), 250
        )
        self.assertGreater(
            min(ImageStat.Stat(picture.crop((135, 140, 185, 178))).mean), 250
        )
        self.assertEqual(plain(report["steps"][0]["title"]), "Example")
        self.assertIn("video.mp4", report["artifacts"])

    def test_failed_assertion_never_produces_passing_report_or_step(self) -> None:
        camera, guide = self.guide()

        def run(camera: Camera, guide: Guide[Camera]) -> None:
            with guide.step("Will fail"):
                raise AssertionError("wrong setting")

        with self.assertRaisesRegex(AssertionError, "wrong setting"):
            guide.run(run)
        report = json.loads((guide.output / "guide.json").read_text())
        self.assertFalse(report["passed"])
        self.assertEqual(report["steps"], [])
        self.assertFalse((guide.output / "screenshots").exists())

    def test_fixture_suppresses_coverage(self) -> None:
        camera, guide = self.guide()

        def run(camera: Camera, guide: Guide[Camera]) -> None:
            with guide.fixture("Prepare settings"):
                camera.press()
            with guide.step("Example", hold=0):
                pass

        with (
            patch.object(Video, "finish"),
            patch.object(Video, "still"),
            patch.object(Video, "_write"),
        ):
            report = guide.run(run)
        self.assertEqual(report["interactions"], [])
        self.assertTrue(report["fixtures"][0]["passed"])

    def test_nested_steps_refused(self) -> None:
        camera, guide = self.guide()
        with (
            guide.step("Outer"),
            self.assertRaisesRegex(ValueError, "nest"),
            guide.step("Inner"),
        ):
            pass
        guide.video.finish()

    def test_invalid_hold(self) -> None:
        camera, guide = self.guide()
        for hold in (-1, float("nan"), float("inf"), True, "2"):
            with self.assertRaises(ValueError), guide.step("Invalid", hold=hold):  # ty: ignore[invalid-argument-type]  # Deliberately invalid input: verify runtime rejection.
                pass

    def test_timestamp_cadence_preserves_short_samples(self) -> None:
        camera, guide = self.guide()
        with patch.object(Video, "_write") as write:
            guide.input("right", True, 0)
            for n in range(101):
                guide.frame(camera.png, n / 100)
            self.assertEqual(write.call_count, 15)

    def test_nonmonotonic_video_time_refused(self) -> None:
        camera, guide = self.guide()
        guide.frame(camera.png, 1)
        with self.assertRaises(ValueError):
            guide.frame(camera.png, 0.5)

    def test_relative_helpers_and_duplicate_function_alias(self) -> None:
        (self.root / "helpers.py").write_text('TITLE="Helper title"\n')
        source = self.root / "example.py"
        source.write_text(
            "from .helpers import TITLE\nfrom fujimod.walkthroughs import walkthrough\n"
            "@walkthrough(TITLE)\ndef first(camera,guide): pass\nalias=first\n"
        )
        found = discover(source)
        self.assertEqual(
            [(w.name, w.title) for w in found], [("first", "Helper title")]
        )

    def test_discovery_does_not_reuse_same_size_timestamp_bytecode(self) -> None:
        import os

        source = self.root / "example.py"
        source.write_text(
            'from fujimod.walkthroughs import walkthrough\n@walkthrough("One")\ndef first(c,g): pass\n'
        )
        self.assertEqual(discover(source)[0].title, "One")
        stamp = source.stat()
        source.write_text(source.read_text().replace('"One"', '"Two"'))
        os.utime(source, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
        self.assertEqual(discover(source)[0].title, "Two")

    def test_input_edges_do_not_retroactively_overlay_prior_frames(self) -> None:
        camera, guide = self.guide()
        video = guide.video
        observed = []

        def write(raw: bytes) -> None:
            observed.append({b["name"] for b in video.badges if b["released"] is None})
            video.frames += 1

        with patch.object(video, "_write", side_effect=write):
            guide.frame(camera.png, 0)
            guide.input("right", True, 0.5)
            guide.frame(camera.png, 1)
        self.assertEqual(len(observed), 13)
        self.assertTrue(all(not labels for labels in observed[:6]))
        self.assertTrue(all(labels == {"right"} for labels in observed[6:]))

    def test_zero_hold_adds_no_frame(self) -> None:
        camera, guide = self.guide()
        with patch.object(guide.video, "_write") as write:
            guide.video.still(camera.png, 0)
            write.assert_not_called()

    def test_input_release_and_fixture_input_cannot_earn_coverage(self) -> None:
        camera, guide = self.guide()
        guide._recording = True
        release = guide.input("right", False, 0)
        with self.assertRaises(ValueError):
            guide.interaction("menu", "m/item", input_index=release)
        with guide.fixture("Setup"):
            setup = guide.input("right", True, 0)
        with self.assertRaises(ValueError):
            guide.interaction("menu", "m/item", input_index=setup)

    def test_catching_a_failed_step_does_not_make_run_pass(self) -> None:
        camera, guide = self.guide()

        def run(c: Camera, g: Guide[Camera]) -> None:
            try:
                with g.step("Broken"):
                    raise AssertionError("broken")
            except AssertionError:
                pass
            with g.step("Later", hold=0):
                pass

        with self.assertRaisesRegex(AssertionError, "broken"):
            guide.run(run)
        self.assertFalse(
            json.loads((guide.output / "guide.json").read_text())["passed"]
        )

    def test_final_observation_is_encoded_and_finish_is_idempotent(self) -> None:
        from unittest.mock import Mock

        camera, guide = self.guide()
        video = guide.video
        observed = []

        def write(raw: bytes) -> None:
            observed.append(raw)
            video.frames += 1
            video.process = Mock()
            video.process.wait.return_value = 0
            video.log = Mock()

        with patch.object(video, "_write", side_effect=write):
            video.frame(camera.png, 0)
            video.frame(b"last observation", 1)
            video.finish()
            count = len(observed)
            video.finish()
        self.assertEqual(observed[-1], b"last observation")
        self.assertEqual(len(observed), count)

    def test_helper_rewrite_with_unchanged_timestamp_is_not_cached(self) -> None:
        import os

        helper = self.root / "helpers.py"
        helper.write_text('TITLE="One"\n')
        source = self.root / "example.py"
        source.write_text(
            "from .helpers import TITLE\nfrom fujimod.walkthroughs import walkthrough\n"
            "@walkthrough(TITLE)\ndef first(camera,guide): pass\n"
        )
        self.assertEqual(discover(source)[0].title, "One")
        stamp = helper.stat()
        helper.write_text('TITLE="Two"\n')
        os.utime(helper, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
        self.assertEqual(discover(source)[0].title, "Two")

    def test_directory_discovery_preserves_cross_file_parent_and_factory_binding(
        self,
    ) -> None:
        (self.root / "navigation.py").write_text(
            "from fujimod.walkthroughs import open_menu\n"
            'page = open_menu("item", title="Open page")\n'
        )
        (self.root / "features.py").write_text(
            "from .navigation import page\n"
            "from fujimod.walkthroughs import walkthrough\n"
            '@walkthrough("Child", start_from=page)\ndef child(c,g): pass\n'
        )
        found = {w.name: w for w in discover(self.root)}
        self.assertEqual(set(found), {"page", "child"})
        self.assertIs(found["child"].start_from, found["page"].function)
        self.assertEqual(found["page"].source.name, "navigation.py")

    def test_menu_instruction_captures_highlight_before_activation_and_branch_starts_after(
        self,
    ) -> None:
        source = self.root / "navigation.py"
        source.write_text(
            "from fujimod.walkthroughs import open_menu\n"
            'page = open_menu("item", title="Item settings")\n'
        )
        visit = discover(source)[0].function

        def make_camera() -> Camera:
            camera = Camera()
            camera.menu_reference = lambda item: MenuEntry(
                "item", "ITEM", ancestors=(MenuEntry("root", "MENU"),)
            )
            camera.select_menu_item = lambda item: setattr(camera, "png", png("blue"))

            def activate(item: MenuReference) -> None:
                self.assertEqual(camera.png, png("blue"))
                camera.press()
                camera.png = png("red")
                camera.guide.frame(camera.png, 0.2)

            camera.open_menu_item = activate
            return camera

        parent = Guide(make_camera(), self.root / "parent", identity={}, title="Parent")
        report = parent.run(visit)
        self.assertEqual(
            [plain(s["title"]) for s in report["steps"]], ["Choose MENU > ITEM"]
        )
        self.assertEqual(
            (parent.output / report["steps"][0]["screenshot"]).read_bytes(), png("blue")
        )
        self.assertEqual(parent.video.last_png, png("red"))
        self.assertGreater(
            report["presentation"]["duration_seconds"], report["steps"][0]["video_end"]
        )
        child = Guide(make_camera(), self.root / "child", identity={}, title="Child")

        def continuation(camera: Camera, guide: Guide[Camera]) -> None:
            with guide.step("Item settings"):
                self.assertEqual(camera.png, png("red"))

        child.run(continuation, prerequisites=[("mod/navigation/page", visit)])
        self.assertEqual((child.output / "poster.png").read_bytes(), png("red"))

    def test_branches_replay_parent_without_inheriting_steps_or_coverage(self) -> None:
        for number in range(2):
            camera = Camera()
            camera.value = 0
            guide = Guide(camera, self.root / str(number), identity={}, title="Child")

            def parent(c: Camera, g: Guide[Camera]) -> None:
                self.assertEqual(c.value, 0)
                with g.step("Parent"):
                    c.press()
                    c.value = 1
                    c.png = png("blue")

            def child(c: Camera, g: Guide[Camera]) -> None:
                self.assertEqual(c.value, 1)
                with g.step("Child", hold=0):
                    c.press()
                    c.value = 2
                    c.png = png("red")

            report = guide.run(child, prerequisites=[("mod/file/parent", parent)])
            self.assertEqual([plain(s["title"]) for s in report["steps"]], ["Child"])
            self.assertEqual(len(report["interactions"]), 1)
            self.assertEqual(len(report["inputs"]), 2)
            self.assertEqual(report["inputs"][0]["seconds"], 0)
            self.assertTrue(report["prerequisites"][0]["passed"])
            self.assertEqual((guide.output / "poster.png").read_bytes(), png("blue"))
            self.assertEqual(
                (guide.output / "screenshots/001.png").read_bytes(), png("red")
            )

    def test_failed_prerequisite_does_not_enter_child(self) -> None:
        from unittest.mock import Mock

        camera, guide = self.guide()
        child = Mock()

        def parent(c: Camera, g: Guide[Camera]) -> None:
            with g.step("Parent"):
                raise AssertionError("parent failed")

        with self.assertRaisesRegex(AssertionError, "parent failed"):
            guide.run(child, prerequisites=[("parent", parent)])
        child.assert_not_called()
        self.assertFalse(guide.report["prerequisites"][0]["passed"])
        self.assertFalse(guide.report["passed"])

    def test_idle_compression_preserves_changed_frames_and_guest_timestamps(
        self,
    ) -> None:
        camera, guide = self.guide()
        video = guide.video
        seen = []

        def write(raw: bytes) -> None:
            seen.append(raw)
            video.frames += 1

        with patch.object(video, "_write", side_effect=write):
            video.frame(camera.png, 0)
            video.frame(camera.png, 3)
            video.frame(png("blue"), 3.1)
            video.frame(png("blue"), 4.1)
        self.assertAlmostEqual(video.skipped_seconds, 3.3)
        self.assertEqual(video.last_time, 4.1)
        self.assertIn(png("blue"), seen)
        self.assertEqual(len(seen), 12)

    def test_explicit_holds_are_not_compressed(self) -> None:
        camera, guide = self.guide()
        video = guide.video

        def write(raw: bytes) -> None:
            video.frames += 1

        with patch.object(video, "_write", side_effect=write):
            video.still(camera.png, 2)
        self.assertEqual(video.frames, 30)

    def test_action_holds_only_present_the_result_and_replay_has_no_hold(self) -> None:
        camera, guide = self.guide()
        from unittest.mock import Mock

        camera.capture = Mock(return_value=camera.png)
        with patch.object(guide.video, "still") as still:
            with guide.action(navigation=True):
                pass
            self.assertEqual(still.call_args.args, (camera.png, 0.12))
            with guide.action():
                pass
            self.assertEqual(still.call_args.args, (camera.png, 0.45))
            guide._preparing = True
            with guide.action():
                pass
            self.assertEqual(still.call_count, 2)
            self.assertEqual(camera.capture.call_count, 2)
        self.assertIsNone(guide.video.last_time)

    def test_failed_action_never_adds_a_presentation_hold(self) -> None:
        camera, guide = self.guide()
        with patch.object(guide.video, "still") as still:
            with self.assertRaises(AssertionError), guide.action():
                raise AssertionError("input failed")
            still.assert_not_called()

    def test_repeated_presses_stay_visible_for_five_seconds_after_release(self) -> None:
        camera, guide = self.guide()
        video = guide.video
        video.input("down", True, 0)
        video.input("down", False, 0)
        video.frames = 2
        video.input("down", True, 0)
        self.assertEqual(video._labels(), [("down", 255), ("down", 255)])
        video.input("down", False, 0)
        video.frames = 4 * video.fps
        self.assertEqual(video._labels(), [("down", 255), ("down", 255)])
        video.frames = 71
        self.assertEqual(video._labels(), [("down", 170), ("down", 255)])
        video.frames = 5 * video.fps
        self.assertEqual(video._labels(), [("down", 85)])
        video.frames = 5 * video.fps + 2
        self.assertEqual(video._labels(), [])

    def test_stack_retains_five_most_recent_presses_in_order(self) -> None:
        camera, guide = self.guide()
        video = guide.video
        names = ["up", "down", "left", "right", "q", "menu_ok"]
        for name in names:
            video.input(name, True, 0)
            video.input(name, False, 0)
        self.assertEqual([name for name, alpha in video._labels()], names[-5:])
