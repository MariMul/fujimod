"""Control identities and badge drawing shared with the web simulator.

controls.json ships with the Python package and is imported directly by Vite.
It owns visual glyphs, accessible names, and badge dimensions/colors.
"""
from importlib.resources import files
from functools import lru_cache
import json
import math

SPEC = json.loads(files(__package__).joinpath('controls.json').read_text())


def control(name):
    try:
        return SPEC['controls'][name]
    except KeyError:
        raise ValueError(f'Missing input presentation: {name}') from None


@lru_cache(maxsize=1)
def _badge_font():
    from PIL import ImageFont
    return ImageFont.load_default(size=SPEC['badge']['font_size'])


def draw_badges(picture, labels, *, font=None, align='center', layout='rows'):
    """Return an annotated copy; labels are (canonical input ID, opacity)."""
    from PIL import Image, ImageDraw
    if align not in ('center', 'right'):
        raise ValueError('Badge alignment must be center or right')
    if layout not in ('rows', 'stack'):
        raise ValueError('Badge layout must be rows or stack')
    style = SPEC['badge']
    font = font or _badge_font()
    layer = Image.new('RGBA', picture.size)
    draw = ImageDraw.Draw(layer)
    icon_size = style['icon_size']
    height = icon_size + 2*style['padding_y'] + 2*style['border_width']
    entries = []
    for name, alpha in labels:
        item = control(name)
        text = item.get('text', item['label'])
        width = icon_size if 'icon' in item else math.ceil(draw.textlength(text, font=font))
        entries.append((item, text, alpha, width+2*style['padding_x']+2*style['border_width']))
    # Center each row, wrapping simultaneous controls rather than clipping text.
    rows = [[]]
    for entry in entries:
        row = rows[-1]
        if row and (layout == 'stack' or
                    sum(e[3] for e in row) + style['gap']*len(row) + entry[3] > picture.width-24):
            rows.append([])
        rows[-1].append(entry)
    y = picture.height-style['bottom']-height*len(rows)-style['gap']*(len(rows)-1)
    def color(value, opacity):
        from PIL.ImageColor import getcolor
        r,g,b,a = getcolor(value, 'RGBA')
        return r,g,b,round(a*opacity/255)
    for row in rows:
        row_width = sum(e[3] for e in row)+style['gap']*max(0,len(row)-1)
        x = (picture.width-row_width)/2 if align == 'center' else picture.width-row_width-12
        for item, text, alpha, width in row:
            draw.rounded_rectangle((x,y,x+width-1,y+height-1), radius=style['radius'],
                fill=color(style['background'],alpha), outline=color(style['border'],alpha),
                width=style['border_width'])
            left, top = x+style['padding_x']+style['border_width'], y+style['padding_y']+style['border_width']
            foreground = color(style['foreground'],alpha)
            if 'icon' in item:
                shape = SPEC['icons'][item['icon']]
                scale = icon_size/24
                for line in shape.get('lines', []):
                    draw.line([(left+px*scale,top+py*scale) for px,py in line], fill=foreground,
                              width=2, joint='curve')
                if 'circle' in shape:
                    cx,cy,r = shape['circle']
                    draw.ellipse((left+(cx-r)*scale,top+(cy-r)*scale,
                                  left+(cx+r)*scale,top+(cy+r)*scale),fill=foreground)
            else:
                draw.text((left,top+icon_size/2),text,font=font,fill=foreground,anchor='lm')
            x += width+style['gap']
        y += height+style['gap']
    return Image.alpha_composite(picture.convert('RGBA'),layer)
