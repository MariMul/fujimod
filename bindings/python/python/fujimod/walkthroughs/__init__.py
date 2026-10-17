"""Executable illustrated guides. Camera adapters own execution and observation."""
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import importlib.util
import importlib.abc
import importlib.machinery
import itertools
import types
import os
import json
import math
from pathlib import Path
import sys


def walkthrough(title):
    """Mark a two-argument (camera, guide) function for discovery."""
    if not isinstance(title, str) or not title.strip():
        raise ValueError('A walkthrough needs a title')
    def decorate(function):
        function.walkthrough_title = title
        return function
    return decorate


@dataclass(frozen=True)
class Walkthrough:
    source: Path
    name: str
    title: str
    function: object


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
    """Import trusted source afresh, with relative sibling-helper imports."""
    path = Path(path).resolve()
    package = '_walkthrough_' + hashlib.sha256(str(path.parent).encode()).hexdigest() + '_' + str(next(_discoveries))
    parent = types.ModuleType(package)
    parent.__path__ = [str(path.parent)]
    parent.__package__ = package
    sys.modules[package] = parent
    name = package + '.' + path.stem
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        # Do not reuse a timestamp/size pyc for rapidly rewritten walkthroughs.
        exec(compile(path.read_bytes(), str(path), 'exec'), module.__dict__)
    except BaseException:
        for key in list(sys.modules):
            if key == package or key.startswith(package + '.'):
                sys.modules.pop(key, None)
        raise
    found = {}
    for value in vars(module).values():
        if callable(value) and hasattr(value, 'walkthrough_title') and value.__module__ == name:
            found[value.__name__] = Walkthrough(path, value.__name__, value.walkthrough_title, value)
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
    def __init__(self, camera, output, *, identity, title, fps=15):
        if not __debug__:
            raise RuntimeError('Walkthroughs require Python assertions')
        from .media import Video
        self.camera, self.output = camera, Path(output)
        self.video = Video(self.output, fps=fps)
        self.output.mkdir(parents=True, exist_ok=False)
        self.report = dict(schema=1, identity=identity, title=title, passed=False,
                           steps=[], inputs=[], interactions=[], fixtures=[])
        self._inside_step = False
        self._recording = False
        self._used = False
        self._failures = []

    def save(self):
        temporary = self.output / '.guide.json.tmp'
        temporary.write_text(json.dumps(self.report, indent=2) + '\n')
        os.replace(temporary, self.output / 'guide.json')

    def frame(self, png, seconds):
        self.video.frame(png, seconds)

    def input(self, name, pressed, seconds):
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
    def step(self, title, text='', *, hold=2):
        if self._inside_step:
            raise ValueError('Guide steps cannot nest')
        if not isinstance(title, str) or not title.strip() or not isinstance(text, str):
            raise ValueError('Step title/text must be strings; title cannot be empty')
        if isinstance(hold, bool) or not isinstance(hold, (int, float)) or not math.isfinite(hold) or hold < 0:
            raise ValueError('hold must be a finite nonnegative number of seconds')
        self._inside_step = True
        start = self.video.seconds
        print(f'Guide step {len(self.report["steps"])+1}: {title}', flush=True)
        try:
            yield
            png = self.camera.capture()
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

    def run(self, function):
        if self._used:
            raise RuntimeError('A Guide can run only once')
        self._used = True
        error = None
        try:
            self.camera.start_recording(self)
            self._recording = True
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
            artifacts = {str(p.relative_to(self.output)): _digest(p)
                         for p in self.output.rglob('*') if p.is_file() and p.name not in ('guide.json', '.guide.json.tmp')}
            self.report['artifacts'] = artifacts
            self.save()
        if error is not None:
            raise error
        return self.report
