"""Native badge pixels use shared control identities and vector directions."""
from io import BytesIO
import unittest

from fujimod.presentation import SPEC, control
from fujimod.recording import Frame
from PIL import Image, ImageChops


def picture(labels: list[tuple[str, int]], width: int = 320, height: int = 240) -> Image.Image:
    frame = Frame.rgb(width, height, bytes([255]) * width * height * 3)
    return Image.open(BytesIO(frame.annotated(labels).png())).convert("RGB")


class InputPresentationTests(unittest.TestCase):
    def test_separate_exposure_and_focus_lock_buttons_have_distinct_badges(self) -> None:
        self.assertEqual(control("ae_lock")["label"], "AE-L")
        self.assertEqual(control("af_lock")["label"], "AF-L")
        self.assertIsNotNone(ImageChops.difference(picture([("ae_lock", 255)]), picture([("af_lock", 255)])).getbbox())

    def test_direction_icons_and_accessible_names(self) -> None:
        for name in ("up", "down", "left", "right"):
            item = control(name)
            self.assertEqual(item["label"], name.title())
            self.assertEqual(item["icon"], name)
            self.assertNotIn("text", item)
        with self.assertRaisesRegex(ValueError, "Missing input presentation"):
            control("right / flash")
        self.assertIsNotNone(ImageChops.difference(picture([("right", 255)]), picture([("left", 255)])).getbbox())

    def test_simultaneous_keys_remain_inside_frame(self) -> None:
        output = picture([("menu_ok", 255), ("command_right", 255), ("right", 255)])
        bounds = ImageChops.difference(Image.new("RGB", output.size, "white"), output).getbbox()
        assert bounds is not None
        self.assertGreater(bounds[0], 0)
        self.assertLess(bounds[2], output.width)
        self.assertLessEqual(bounds[3], output.height - SPEC["badge"]["bottom"])

    def test_stack_is_vertical_at_bottom_right_with_newest_at_bottom(self) -> None:
        output = picture([("down", 64), ("menu_ok", 255)], 720, 480)
        bounds = ImageChops.difference(Image.new("RGB", output.size, "white"), output).getbbox()
        height = SPEC["badge"]["icon_size"] + 2 * SPEC["badge"]["padding_y"] + 2 * SPEC["badge"]["border_width"]
        assert bounds is not None
        self.assertEqual(bounds[3], 480 - SPEC["badge"]["bottom"])
        self.assertEqual(bounds[3] - bounds[1], 2 * height + SPEC["badge"]["gap"])
        self.assertEqual(bounds[2], 708)
        newest = output.getpixel((700, 400))
        oldest = output.getpixel((700, 360))
        assert isinstance(newest, tuple) and isinstance(oldest, tuple)
        self.assertLess(newest[1], 100)
        self.assertGreater(oldest[1], 150)
