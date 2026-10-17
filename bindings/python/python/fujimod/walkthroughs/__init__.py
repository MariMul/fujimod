"""Executable illustrated guides. Camera adapters own execution and observation."""
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import importlib.util
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


def discover(path):
    """Import one trusted local Python file, preserving ordinary Python imports."""
    path = Path(path).resolve()
    name = '_walkthrough_' + hashlib.sha256(str(path).encode()).hexdigest()
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(name, None)
        raise
    return [Walkthrough(path, name, value.walkthrough_title, value)
            for name, value in vars(module).items()
            if callable(value) and hasattr(value, 'walkthrough_title')
            and value.__module__ == module.__name__]


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
        self.output.mkdir(parents=True, exist_ok=False)
        self.report = dict(schema=1, identity=identity, title=title, passed=False,
                           steps=[], inputs=[], interactions=[], fixtures=[])
        self.video = Video(self.output, fps=fps)
        self._inside_step = False
        self._recording = False

    def frame(self, png, seconds):
        self.video.frame(png, seconds)

    def input(self, name, pressed, seconds):
        event = dict(name=name, pressed=pressed, seconds=seconds,
                     video_seconds=self.video.seconds)
        self.report['inputs'].append(event)
        self.video.input(name, pressed)
        return len(self.report['inputs']) - 1

    def interaction(self, surface, item, *, input_index, **evidence):
        if not self._recording:
            return
        if not 0 <= input_index < len(self.report['inputs']):
            raise ValueError('Interaction needs an observed input')
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
            (self.output / 'guide.json').write_text(json.dumps(self.report, indent=2) + '\n')
        finally:
            self._inside_step = False

    def run(self, function):
        error = None
        try:
            self.camera.start_recording(self)
            self._recording = True
            function(self.camera, self)
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
            artifacts = {str(p.relative_to(self.output)): hashlib.sha256(p.read_bytes()).hexdigest()
                         for p in self.output.rglob('*') if p.is_file() and p.name != 'guide.json'}
            self.report['artifacts'] = artifacts
            (self.output / 'guide.json').write_text(json.dumps(self.report, indent=2) + '\n')
        if error is not None:
            raise error
        return self.report
