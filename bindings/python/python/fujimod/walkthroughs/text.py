"""Semantic walkthrough text using Python's standard template strings.

Adapters resolve menu entries. The recorder exports resolved tokens and PNGs;
consumers need neither Python nor firmware to render the resulting document.
"""
from dataclasses import dataclass, replace
import hashlib
import html
from pathlib import PurePosixPath
from string.templatelib import Template, convert


@dataclass(frozen=True)
class MenuReference:
    menu: str | int
    key: int | None = None
    display: str = 'full'
    via: 'MenuReference | None' = None

    def __post_init__(self):
        if not ((isinstance(self.menu, str) and self.menu) or type(self.menu) is int and self.menu >= 0):
            raise ValueError('menu must be a symbol name or nonnegative menu code')
        if self.key is not None and (type(self.key) is not int or self.key < 0):
            raise ValueError('Menu entry key must be a nonnegative integer')
        if self.display not in ('full', 'name', 'icon'):
            raise ValueError('Unknown menu presentation')
        if self.via is not None and not isinstance(self.via, MenuReference):
            raise TypeError('via must reference the row that opens this menu')

    @property
    def name(self):
        return replace(self, display='name')

    @property
    def icon(self):
        return replace(self, display='icon')

    def path(self, *, relative_to=None):
        if relative_to is not None and not isinstance(relative_to, MenuReference):
            raise TypeError('relative_to must be a menu reference')
        return MenuPath(replace(self, display='full'), relative_to)

    def __str__(self):
        raise TypeError('Menu references need a t-string, not an f-string')


def ref(*, menu, key=None, via=None):
    """Reference a declaration or stock menu; key selects a row in that menu."""
    return MenuReference(menu, key, via=via)


@dataclass(frozen=True)
class MenuPath:
    item: MenuReference
    relative_to: MenuReference | None = None

    def __str__(self):
        raise TypeError('Menu paths need a t-string, not an f-string')


@dataclass(frozen=True)
class MenuEntry:
    identity: str
    name: str
    icon: bytes | None = None
    ancestors: tuple = ()


def encode(value, *, resolve, output):
    """Resolve str/Template into JSON tokens, retaining menu identity."""
    def menu(entry, display='full'):
        token = dict(type='menu', identity=entry.identity, name=entry.name, display=display, icon=None)
        if entry.icon is not None and display != 'name':
            if not entry.icon.startswith(b'\x89PNG\r\n\x1a\n'):
                raise ValueError('Menu icons must be PNG bytes')
            digest = hashlib.sha256(entry.icon).hexdigest()
            asset = 'icons/' + digest + '.png'
            destination = output / asset
            destination.parent.mkdir(exist_ok=True)
            if destination.exists() and destination.read_bytes() != entry.icon:
                raise ValueError('Conflicting menu icon asset')
            destination.write_bytes(entry.icon)
            token['icon'] = dict(asset=asset, sha256=digest)
        return token

    def interpolation(item):
        if isinstance(item, MenuReference):
            entry = resolve(item)
            if not entry.name and not entry.icon:
                raise ValueError('Menu has no visible caption; reference a displayed row')
            return menu(entry, item.display)
        if isinstance(item, MenuPath):
            entry = resolve(item.item)
            entries = [*entry.ancestors, entry]
            if item.relative_to is not None:
                ancestor = resolve(item.relative_to)
                positions = [i for i, part in enumerate(entries[:-1]) if part.identity == ancestor.identity]
                if len(positions) != 1:
                    raise ValueError('Relative menu path must name a unique ancestor')
                entries = entries[positions[0] + 1:]
            entries = [part for part in entries if part.name or part.icon]
            if not entries:
                raise ValueError('Menu path has no visible entries')
            return dict(type='menu_path', items=[menu(part) for part in entries])
        raise TypeError('Unknown semantic walkthrough value')

    if isinstance(value, str):
        return [dict(type='text', text=value)] if value else []
    if not isinstance(value, Template):
        raise TypeError('Walkthrough text must be str or a t-string')
    tokens = []
    for part in value:
        if isinstance(part, str):
            tokens.append(dict(type='text', text=part))
        else:
            if isinstance(part.value, (MenuReference, MenuPath, Template)):
                if part.conversion or part.format_spec:
                    raise ValueError('Menu references use properties, not conversions or format specs')
                if isinstance(part.value, Template):
                    tokens.extend(encode(part.value, resolve=resolve, output=output))
                else:
                    tokens.append(interpolation(part.value))
            else:
                tokens.append(dict(type='text', text=format(convert(part.value, part.conversion), part.format_spec)))
    return tokens


def plain(tokens):
    return ''.join(token['text'] if token['type'] == 'text' else
                   ' > '.join(item['name'] for item in token['items']) if token['type'] == 'menu_path' else
                   token['name'] for token in tokens)


def icons(tokens):
    """Yield all icon descriptors; malformed token types are refused."""
    if not isinstance(tokens, list):
        raise ValueError('Walkthrough text must be a token array')
    for token in tokens:
        kind = token['type']
        if kind == 'text':
            if not isinstance(token['text'], str):
                raise ValueError('Invalid text token')
        elif kind == 'menu_path':
            if not token['items'] or any(item['type'] != 'menu' for item in token['items']):
                raise ValueError('Invalid menu path')
            yield from icons(token['items'])
        elif kind == 'menu':
            if (not isinstance(token['identity'], str) or not token['identity'] or
                    not isinstance(token['name'], str) or token['display'] not in ('full', 'name', 'icon')):
                raise ValueError('Invalid menu token')
            if token['icon'] is not None:
                icon = token['icon']
                path = PurePosixPath(icon['asset'])
                if (str(path) != icon['asset'] or path.is_absolute() or '..' in path.parts or
                        len(path.parts) != 2 or path.parts[0] != 'icons' or path.suffix != '.png' or
                        not isinstance(icon['sha256'], str) or len(icon['sha256']) != 64):
                    raise ValueError('Invalid menu icon asset')
                yield icon
        else:
            raise ValueError('Unknown walkthrough text token: ' + str(kind))


def render(tokens, *, prefix=''):
    """Render validated semantic text as escaped HTML, using bundled assets."""
    list(icons(tokens))
    def token_html(token):
        if token['type'] == 'text':
            return html.escape(token['text'])
        if token['type'] == 'menu_path':
            return '<span class="menu-path">' + '<span aria-hidden="true"> › </span>'.join(
                token_html(item) for item in token['items']) + '</span>'
        label = html.escape(token['name'])
        image = ''
        if token['icon'] is not None and token['display'] != 'name':
            alt = label if token['display'] == 'icon' else ''
            image = f'<img class="menu-icon" src="{html.escape(prefix + token["icon"]["asset"], quote=True)}" alt="{alt}">'
        text = label if token['display'] != 'icon' or not image else ''
        return f'<span class="menu-ref" data-menu="{html.escape(token["identity"], quote=True)}">{image}{text}</span>'
    return ''.join(token_html(token) for token in tokens)
