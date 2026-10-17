"""The recording uses shared vector glyphs rather than font-dependent arrows."""
from PIL import Image, ImageChops
from fujimod.presentation import SPEC, control, draw_badges
import unittest

class InputPresentationTests(unittest.TestCase):
    def test_direction_icons_and_accessible_names(self):
        for name in ('up', 'down', 'left', 'right'):
            item = control(name)
            self.assertEqual(item['label'], name.title())
            self.assertEqual(item['icon'], name)
            self.assertNotIn('text', item)
        with self.assertRaisesRegex(ValueError, 'Missing input presentation'):
            control('right / flash')

    def test_directions_have_distinct_pixels_without_font_glyphs(self):
        class NoTextFont:
            def getmask(self, *args, **kwargs):
                raise AssertionError('Directions must be vector icons')
        blank = Image.new('RGBA', (320, 240), 'white')
        right = draw_badges(blank, [('right', 255)], font=NoTextFont())
        left = draw_badges(blank, [('left', 255)], font=NoTextFont())
        self.assertIsNotNone(ImageChops.difference(right, left).convert('RGB').getbbox())
        self.assertEqual(blank.getpixel((160, 150)), (255, 255, 255, 255))
        red, green, blue, _ = right.getpixel((140, 150))
        self.assertGreater(green, red)
        self.assertLess(green, 100)

    def test_simultaneous_keys_wrap_inside_frame(self):
        blank = Image.new('RGBA', (320, 240), 'white')
        output = draw_badges(blank, [('menu_ok', 255), ('command_right', 255), ('right', 255)])
        bounds = ImageChops.difference(blank, output).convert('RGB').getbbox()
        self.assertGreater(bounds[0], 0)
        self.assertLess(bounds[2], blank.width)
        self.assertLessEqual(bounds[3], blank.height-SPEC['badge']['bottom'])
