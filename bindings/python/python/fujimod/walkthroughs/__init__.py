"""Executable illustrated guides. Camera adapters own execution and observation."""
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import importlib
import importlib.abc
import importlib.machinery
import itertools
import types
import os
import json
import math
from pathlib import Path
import sys
from .text import MenuReference, encode, plain, ref

SCHEMA = 3


# How a walkthrough is presented in owner documentation. Coverage counts every
# role; the role only decides presentation:
#   guide      the feature's main how-to, shown expanded (one per feature)
#   alternate  another way to do the same task, listed collapsed
#   tour       visits every option; source of option cards (contact sheet)
#   test       behaviour checks, hidden from owner documentation
ROLES = ('guide', 'alternate', 'tour', 'test')


def walkthrough(title, *, start_from=None, role=None, feature=None):
    """Mark a two-argument (camera, guide) function for discovery.

    role is one of ROLES; feature names the owner task the walkthrough
    documents (e.g. 'aspect_ratio'), shared by its guide and alternates.
    Walkthroughs without a role are untriaged."""
    if not isinstance(title, str) or not title.strip():
        raise ValueError('A walkthrough needs a title')
    if start_from is not None and not hasattr(start_from, 'walkthrough_title'):
        raise ValueError('start_from must reference a walkthrough')
    if role is not None and role not in ROLES:
        raise ValueError(f'Walkthrough role must be one of {ROLES}')
    if feature is not None and (not isinstance(feature, str) or not feature.isidentifier()):
        raise ValueError('A walkthrough feature is an identifier, e.g. aspect_ratio')
    def decorate(function):
        function.walkthrough_title = title
        function.walkthrough_start_from = start_from
        function.walkthrough_role = role
        function.walkthrough_feature = feature
        return function
    return decorate


def open_menu(item, *, title, start_from=None, role=None, feature=None):
    """Illustrate the highlighted menu destination before opening its page."""
    item = ref(menu=item) if isinstance(item, str) else item
    if not isinstance(item, MenuReference):
        raise ValueError('A menu starter needs a menu reference or item name')
    @walkthrough(title, start_from=start_from, role=role, feature=feature)
    def visit(camera, guide):
        with guide.step(t'Choose {item.path()}'):
            camera.select_menu_item(item)
        camera.open_menu_item(item)
        guide.pause(1.5)
    # Factory guides take their identity from their binding in the calling
    # module. Imported guides retain their original module ownership.
    visit.walkthrough_module = sys._getframe(1).f_globals['__name__']
    visit.walkthrough_factory = True
    return visit


@dataclass(frozen=True)
class Walkthrough:
    source: Path
    name: str
    title: str
    function: object

    @property
    def start_from(self):
        return self.function.walkthrough_start_from

    @property
    def role(self):
        return getattr(self.function, 'walkthrough_role', None)

    @property
    def feature(self):
        return getattr(self.function, 'walkthrough_feature', None)


class _FreshSourceLoader(importlib.machinery.SourceFileLoader):
    def get_code(self, fullname):
        return compile(self.get_data(self.path), self.path, 'exec')


class _HelperFinder(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.startswith('_walkthrough_') and '.' in fullname:
            spec = importlib.machinery.PathFinder.find_spec(fullname, path)
            if spec is not None and isinstance(spec.loader, importlib.machinery.SourceFileLoader):
                spec.loader = _FreshSourceLoader(fullname, spec.origin)
            return spec
        return None


sys.meta_path.insert(0, _HelperFinder())
_discoveries = itertools.count()


def discover(path):
    """Import one file or a directory in one fresh package, preserving references."""
    path = Path(path).resolve()
    directory = path if path.is_dir() else path.parent
    sources = sorted(p for p in directory.glob('*.py') if not p.name.startswith('_')) if path.is_dir() else [path]
    package = '_walkthrough_' + hashlib.sha256(str(directory).encode()).hexdigest() + '_' + str(next(_discoveries))
    parent = types.ModuleType(package)
    parent.__path__ = [str(directory)]
    parent.__package__ = package
    sys.modules[package] = parent
    try:
        modules = [(source, importlib.import_module(package + '.' + source.stem)) for source in sources]
    except BaseException:
        for key in list(sys.modules):
            if key == package or key.startswith(package + '.'):
                sys.modules.pop(key, None)
        raise
    found = {}
    for source, module in modules:
        for binding, value in vars(module).items():
            if not callable(value) or not hasattr(value, 'walkthrough_title'):
                continue
            if getattr(value, 'walkthrough_module', value.__module__) != module.__name__:
                continue
            name = binding if getattr(value, 'walkthrough_factory', False) else value.__name__
            found.setdefault(value, Walkthrough(source, name, value.walkthrough_title, value))
    return list(found.values())


def _digest(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024*1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


class Guide:
    """One run. Adapter callbacks record frames, input edges and interactions.

    capture() must return settled, clean PNG bytes. start_recording(guide)
    observes all adapter input paths; it must not infer coverage from prose.
    The factory is called once per run and must provide a fresh camera.
    """
    def __init__(self, camera, output, *, identity, title, role=None, feature=None, fps=15,
                 navigation_hold=.12, action_hold=.45):
        if not __debug__:
            raise RuntimeError('Walkthroughs require Python assertions')
        from .media import Video, seconds_value
        self.navigation_hold = seconds_value(navigation_hold)
        self.action_hold = seconds_value(action_hold)
        self.camera, self.output = camera, Path(output)
        self.video = Video(self.output, fps=fps)
        self.output.mkdir(parents=True, exist_ok=False)
        self.report = dict(schema=SCHEMA, identity=identity, title=title, role=role, feature=feature, passed=False,
                           steps=[], inputs=[], interactions=[], fixtures=[], prerequisites=[])
        self._inside_step = False
        self._recording = False
        self._used = False
        self._failures = []
        self._preparing = False

    def save(self):
        temporary = self.output / '.guide.json.tmp'
        temporary.write_text(json.dumps(self.report, indent=2) + '\n')
        os.replace(temporary, self.output / 'guide.json')

    def frame(self, png, seconds):
        if self._preparing:
            return
        if png and 'poster' not in self.report:
            (self.output/'poster.png').write_bytes(png)
            self.report['poster'] = 'poster.png'
        self.video.frame(png, seconds)

    def input(self, name, pressed, seconds):
        if self._preparing:
            return None
        self.video.input(name, pressed, seconds)
        event = dict(name=name, pressed=pressed, seconds=seconds,
                     video_seconds=self.video.seconds, fixture=not self._recording)
        self.report['inputs'].append(event)
        return len(self.report['inputs']) - 1

    def interaction(self, surface, item, *, input_index, **evidence):
        if not self._recording:
            return
        if type(input_index) is not int or not 0 <= input_index < len(self.report['inputs']):
            raise ValueError('Interaction needs an observed input')
        edge = self.report['inputs'][input_index]
        if edge['pressed'] is not True or edge['fixture']:
            raise ValueError('Interaction needs a non-fixture press input')
        if surface not in ('menu', 'q', 'fn') or not isinstance(item, str) or not item:
            raise ValueError('Invalid interaction surface or item')
        self.report['interactions'].append(dict(
            surface=surface, item=item, input_index=input_index,
            step=len(self.report['steps']) + 1 if self._inside_step else None,
            evidence=evidence))

    @contextmanager
    def fixture(self, description):
        """Declare a setup intervention. Observations here cannot earn coverage."""
        previous = self._recording
        self._recording = False
        entry = dict(description=description, passed=False)
        self.report['fixtures'].append(entry)
        try:
            yield
            entry['passed'] = True
        except BaseException as error:
            self._failures.append(error)
            raise
        finally:
            self._recording = previous

    @contextmanager
    def step(self, title, text='', *, hold=1.5):
        if self._inside_step:
            raise ValueError('Guide steps cannot nest')
        resolve = lambda reference: self.camera.menu_reference(reference)
        title = encode(title, resolve=resolve, output=self.output)
        text = encode(text, resolve=resolve, output=self.output)
        if not plain(title).strip():
            raise ValueError('Step title cannot be empty')
        if isinstance(hold, bool) or not isinstance(hold, (int, float)) or not math.isfinite(hold) or hold < 0:
            raise ValueError('hold must be a finite nonnegative number of seconds')
        self._inside_step = True
        start = self.video.seconds
        print(f'Guide step {len(self.report["steps"])+1}: {plain(title)}', flush=True)
        try:
            yield
            png = self.camera.capture()
            if self._preparing:
                return
            number = len(self.report['steps']) + 1
            name = f'screenshots/{number:03d}.png'
            path = self.output / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(png)
            self.video.still(png, hold)
            self.report['steps'].append(dict(
                number=number, title=title, text=text, screenshot=name,
                screenshot_sha256=hashlib.sha256(png).hexdigest(),
                video_start=start, video_end=self.video.seconds, hold=hold))
            self.save()
        except BaseException as error:
            self._failures.append(error)
            raise
        finally:
            self._inside_step = False

    def pause(self, seconds=.35):
        """Show a settled action result without adding an illustrated step."""
        from .media import seconds_value
        seconds_value(seconds)
        png = self.camera.capture()
        if not self._preparing:
            self.video.still(png, seconds)

    @contextmanager
    def action(self, *, navigation=False):
        """Adapter boundary: present a completed input without advancing time.

        Execution readiness belongs to the adapter. These are recording-only
        result holds: short for finding a destination, longer for operating it.
        Prerequisite replay incurs neither hold nor extra capture.
        """
        yield
        if not self._preparing:
            self.video.still(self.camera.capture(), self.navigation_hold if navigation else self.action_hold)

    def run(self, function, *, prerequisites=()):
        if self._used:
            raise RuntimeError('A Guide can run only once')
        self._used = True
        error = None
        try:
            self._preparing = True
            self.camera.start_recording(self)
            for name, prepare in prerequisites:
                self.report['stage'] = 'prerequisite'
                print('Replaying prerequisite: ' + name, flush=True)
                entry = dict(walkthrough=name, passed=False)
                self.report['prerequisites'].append(entry)
                prepare(self.camera, self)
                if self._failures:
                    raise self._failures[0]
                entry['passed'] = True
            self._preparing = False
            self._recording = True
            self.report['stage'] = 'execution'
            self.camera.begin_segment()
            function(self.camera, self)
            if self._failures:
                raise self._failures[0]
            if not self.report['steps']:
                raise ValueError('A walkthrough must produce at least one illustrated step')
        except BaseException as caught:
            error = caught
            self.report['error'] = f'{type(caught).__name__}: {caught}'
        finally:
            self._recording = False
            self._preparing = False
            try:
                self.camera.stop_recording()
            except BaseException as caught:
                if error is None:
                    error = caught
                    self.report['error'] = f'{type(caught).__name__}: {caught}'
            try:
                self.video.finish()
                self.report['video'] = 'video.mp4'
            except BaseException as caught:
                if error is None:
                    error = caught
                    self.report['error'] = f'{type(caught).__name__}: {caught}'
            self.report['passed'] = error is None
            self.report['presentation'] = dict(fps=self.video.fps, idle_limit=self.video.idle_limit,
                skipped_guest_seconds=self.video.skipped_seconds,
                navigation_hold=self.navigation_hold, action_hold=self.action_hold,
                duration_seconds=self.video.seconds)
            artifacts = {str(p.relative_to(self.output)): _digest(p)
                         for p in self.output.rglob('*') if p.is_file() and p.name not in ('guide.json', '.guide.json.tmp')}
            self.report['artifacts'] = artifacts
            self.save()
        if error is not None:
            raise error
        return self.report
