"""Executable illustrated guides. Camera adapters own execution and observation."""

import hashlib
import importlib
import importlib.abc
import importlib.machinery
import itertools
import json
import math
import os
import sys
import types
from collections.abc import Callable, Generator, Sequence
from contextlib import AbstractContextManager, contextmanager
from dataclasses import dataclass
from pathlib import Path
from string.templatelib import Template
from typing import NotRequired, Protocol, Self, TypedDict, cast

from .text import MenuEntry, MenuReference, Token, encode, plain, ref


class RecordingCamera(Protocol):
    def capture(self) -> bytes: ...
    def start_recording(self, guide: Guide[Self]) -> None: ...
    def stop_recording(self) -> None: ...
    def begin_segment(self) -> None: ...


class MenuCamera(Protocol):
    def select_menu_item(self, item: MenuReference, /) -> None: ...
    def open_menu_item(self, item: MenuReference, /) -> None: ...
    def menu_reference(self, item: MenuReference, /) -> MenuEntry: ...


class MenuGuide(Protocol):
    def step(
        self, title: str | Template, text: str | Template = "", *, hold: float = 1.5
    ) -> AbstractContextManager[None]: ...
    def pause(self, seconds: float = 0.35) -> None: ...


class Scenario(Protocol):
    walkthrough_title: str
    walkthrough_start_from: Scenario | None
    walkthrough_module: str
    walkthrough_factory: bool
    __module__: str
    __name__: str

    def __call__(self, camera: MenuCamera, guide: MenuGuide) -> None: ...


class Input(TypedDict):
    name: str
    pressed: bool
    seconds: float
    video_seconds: float
    fixture: bool


class Interaction(TypedDict):
    surface: str
    item: str
    input_index: int
    step: int | None
    evidence: dict[str, object]


class Step(TypedDict):
    number: int
    title: list[Token]
    text: list[Token]
    screenshot: str
    screenshot_sha256: str
    video_start: float
    video_end: float
    hold: float


class Fixture(TypedDict):
    description: str
    passed: bool


class Prerequisite(TypedDict):
    walkthrough: str
    passed: bool


class Presentation(TypedDict):
    fps: int
    idle_limit: float
    skipped_guest_seconds: float
    navigation_hold: float
    action_hold: float
    duration_seconds: float


class Report(TypedDict):
    schema: int
    identity: dict[str, object]
    title: str
    passed: bool
    steps: list[Step]
    inputs: list[Input]
    interactions: list[Interaction]
    fixtures: list[Fixture]
    prerequisites: list[Prerequisite]
    poster: NotRequired[str]
    stage: NotRequired[str]
    error: NotRequired[str]
    video: NotRequired[str]
    presentation: NotRequired[Presentation]
    artifacts: NotRequired[dict[str, str]]


SCHEMA = 3


def walkthrough(
    title: str, *, start_from: Scenario | None = None
) -> Callable[[Callable[..., None]], Scenario]:
    """Mark a two-argument (camera, guide) function for discovery."""
    if not isinstance(title, str) or not title.strip():
        raise ValueError("A walkthrough needs a title")
    if start_from is not None and not hasattr(start_from, "walkthrough_title"):
        raise ValueError("start_from must reference a walkthrough")

    def decorate(function: Callable[..., None]) -> Scenario:
        scenario = cast(Scenario, function)
        scenario.walkthrough_title = title
        scenario.walkthrough_start_from = start_from
        return scenario

    return decorate


def open_menu(
    item: str | MenuReference, *, title: str, start_from: Scenario | None = None
) -> Scenario:
    """Illustrate the highlighted menu destination before opening its page."""
    item = ref(menu=item) if isinstance(item, str) else item
    if not isinstance(item, MenuReference):
        raise ValueError("A menu starter needs a menu reference or item name")

    @walkthrough(title, start_from=start_from)
    def visit(camera: MenuCamera, guide: MenuGuide) -> None:
        with guide.step(t"Choose {item.path()}"):
            camera.select_menu_item(item)
        camera.open_menu_item(item)
        guide.pause(1.5)

    # Factory guides take their identity from their binding in the calling
    # module. Imported guides retain their original module ownership.
    visit.walkthrough_module = sys._getframe(1).f_globals["__name__"]
    visit.walkthrough_factory = True
    return visit


@dataclass(frozen=True)
class Walkthrough:
    source: Path
    name: str
    title: str
    function: Scenario

    @property
    def start_from(self) -> Scenario | None:
        return self.function.walkthrough_start_from


class _FreshSourceLoader(importlib.machinery.SourceFileLoader):
    def get_code(self, fullname: str) -> types.CodeType:
        return compile(self.get_data(self.path), self.path, "exec")


class _HelperFinder(importlib.abc.MetaPathFinder):
    def find_spec(
        self,
        fullname: str,
        path: Sequence[str] | None = None,
        target: types.ModuleType | None = None,
    ) -> importlib.machinery.ModuleSpec | None:
        if fullname.startswith("_walkthrough_") and "." in fullname:
            spec = importlib.machinery.PathFinder.find_spec(fullname, path)
            if (
                spec is not None
                and spec.origin is not None
                and isinstance(spec.loader, importlib.machinery.SourceFileLoader)
            ):
                spec.loader = _FreshSourceLoader(fullname, spec.origin)
            return spec
        return None


sys.meta_path.insert(0, _HelperFinder())
_discoveries = itertools.count()


def discover(path: str | Path) -> list[Walkthrough]:
    """Import one file or a directory in one fresh package, preserving references."""
    path = Path(path).resolve()
    directory = path if path.is_dir() else path.parent
    sources = (
        sorted(p for p in directory.glob("*.py") if not p.name.startswith("_"))
        if path.is_dir()
        else [path]
    )
    package = (
        "_walkthrough_"
        + hashlib.sha256(str(directory).encode()).hexdigest()
        + "_"
        + str(next(_discoveries))
    )
    parent = types.ModuleType(package)
    parent.__path__ = [str(directory)]
    parent.__package__ = package
    sys.modules[package] = parent
    try:
        modules = [
            (source, importlib.import_module(package + "." + source.stem))
            for source in sources
        ]
    except BaseException:
        for key in list(sys.modules):
            if key == package or key.startswith(package + "."):
                sys.modules.pop(key, None)
        raise
    found: dict[Scenario, Walkthrough] = {}
    for source, module in modules:
        for binding, candidate in vars(module).items():
            value = cast(Scenario, candidate)
            if not callable(value) or not hasattr(value, "walkthrough_title"):
                continue
            if (
                getattr(value, "walkthrough_module", value.__module__)
                != module.__name__
            ):
                continue
            name = (
                binding
                if getattr(value, "walkthrough_factory", False)
                else value.__name__
            )
            found.setdefault(
                value, Walkthrough(source, name, value.walkthrough_title, value)
            )
    return list(found.values())


def _digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


class Guide[C: RecordingCamera]:
    """One run. Adapter callbacks record frames, input edges and interactions.

    capture() must return settled, clean PNG bytes. start_recording(guide)
    observes all adapter input paths; it must not infer coverage from prose.
    The factory is called once per run and must provide a fresh camera.
    """

    def __init__(
        self,
        camera: C,
        output: str | Path,
        *,
        identity: dict[str, object],
        title: str,
        fps: int = 15,
        navigation_hold: float = 0.12,
        action_hold: float = 0.45,
    ) -> None:
        if not __debug__:
            raise RuntimeError("Walkthroughs require Python assertions")
        from .media import Video, seconds_value

        self.navigation_hold = seconds_value(navigation_hold)
        self.action_hold = seconds_value(action_hold)
        self.camera, self.output = camera, Path(output)
        self.video = Video(self.output, fps=fps)
        self.output.mkdir(parents=True, exist_ok=False)
        self.report: Report = dict(
            schema=SCHEMA,
            identity=identity,
            title=title,
            passed=False,
            steps=[],
            inputs=[],
            interactions=[],
            fixtures=[],
            prerequisites=[],
        )
        self._inside_step = False
        self._recording = False
        self._used = False
        self._failures: list[BaseException] = []
        self._preparing = False

    def save(self) -> None:
        temporary = self.output / ".guide.json.tmp"
        temporary.write_text(json.dumps(self.report, indent=2) + "\n")
        os.replace(temporary, self.output / "guide.json")

    def frame(self, png: bytes, seconds: float) -> None:
        if self._preparing:
            return
        if png and "poster" not in self.report:
            (self.output / "poster.png").write_bytes(png)
            self.report["poster"] = "poster.png"
        self.video.frame(png, seconds)

    def input(self, name: str, pressed: bool, seconds: float) -> int | None:
        if self._preparing:
            return None
        self.video.input(name, pressed, seconds)
        event: Input = dict(
            name=name,
            pressed=pressed,
            seconds=seconds,
            video_seconds=self.video.seconds,
            fixture=not self._recording,
        )
        self.report["inputs"].append(event)
        return len(self.report["inputs"]) - 1

    def interaction(
        self, surface: str, item: str, *, input_index: int | None, **evidence: object
    ) -> None:
        if not self._recording:
            return
        if type(input_index) is not int or not 0 <= input_index < len(
            self.report["inputs"]
        ):
            raise ValueError("Interaction needs an observed input")
        edge = self.report["inputs"][input_index]
        if edge["pressed"] is not True or edge["fixture"]:
            raise ValueError("Interaction needs a non-fixture press input")
        if surface not in ("menu", "q", "fn") or not isinstance(item, str) or not item:
            raise ValueError("Invalid interaction surface or item")
        self.report["interactions"].append(
            dict(
                surface=surface,
                item=item,
                input_index=input_index,
                step=len(self.report["steps"]) + 1 if self._inside_step else None,
                evidence=evidence,
            )
        )

    @contextmanager
    def fixture(self, description: str) -> Generator[None]:
        """Declare a setup intervention. Observations here cannot earn coverage."""
        previous = self._recording
        self._recording = False
        entry: Fixture = dict(description=description, passed=False)
        self.report["fixtures"].append(entry)
        try:
            yield
            entry["passed"] = True
        except BaseException as error:
            self._failures.append(error)
            raise
        finally:
            self._recording = previous

    @contextmanager
    def step(
        self, title: str | Template, text: str | Template = "", *, hold: float = 1.5
    ) -> Generator[None]:
        if self._inside_step:
            raise ValueError("Guide steps cannot nest")

        def resolve(reference: MenuReference) -> MenuEntry:
            return cast(MenuCamera, self.camera).menu_reference(reference)

        title_tokens = encode(title, resolve=resolve, output=self.output)
        text_tokens = encode(text, resolve=resolve, output=self.output)
        if not plain(title_tokens).strip():
            raise ValueError("Step title cannot be empty")
        if (
            isinstance(hold, bool)
            or not isinstance(hold, (int, float))
            or not math.isfinite(hold)
            or hold < 0
        ):
            raise ValueError("hold must be a finite nonnegative number of seconds")
        self._inside_step = True
        start = self.video.seconds
        print(
            f"Guide step {len(self.report['steps']) + 1}: {plain(title_tokens)}",
            flush=True,
        )
        try:
            yield
            png = self.camera.capture()
            if self._preparing:
                return
            number = len(self.report["steps"]) + 1
            name = f"screenshots/{number:03d}.png"
            path = self.output / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(png)
            self.video.still(png, hold)
            self.report["steps"].append(
                dict(
                    number=number,
                    title=title_tokens,
                    text=text_tokens,
                    screenshot=name,
                    screenshot_sha256=hashlib.sha256(png).hexdigest(),
                    video_start=start,
                    video_end=self.video.seconds,
                    hold=hold,
                )
            )
            self.save()
        except BaseException as error:
            self._failures.append(error)
            raise
        finally:
            self._inside_step = False

    def pause(self, seconds: float = 0.35) -> None:
        """Show a settled action result without adding an illustrated step."""
        from .media import seconds_value

        seconds_value(seconds)
        png = self.camera.capture()
        if not self._preparing:
            self.video.still(png, seconds)

    @contextmanager
    def action(self, *, navigation: bool = False) -> Generator[None]:
        """Adapter boundary: present a completed input without advancing time.

        Execution readiness belongs to the adapter. These are recording-only
        result holds: short for finding a destination, longer for operating it.
        Prerequisite replay incurs neither hold nor extra capture.
        """
        yield
        if not self._preparing:
            self.video.still(
                self.camera.capture(),
                self.navigation_hold if navigation else self.action_hold,
            )

    def run(
        self,
        function: Callable[[C, Guide[C]], None],
        *,
        prerequisites: Sequence[tuple[str, Callable[[C, Guide[C]], None]]] = (),
    ) -> Report:
        if self._used:
            raise RuntimeError("A Guide can run only once")
        self._used = True
        error = None
        try:
            self._preparing = True
            self.camera.start_recording(self)
            for name, prepare in prerequisites:
                self.report["stage"] = "prerequisite"
                print("Replaying prerequisite: " + name, flush=True)
                entry: Prerequisite = dict(walkthrough=name, passed=False)
                self.report["prerequisites"].append(entry)
                prepare(self.camera, self)
                if self._failures:
                    raise self._failures[0]
                entry["passed"] = True
            self._preparing = False
            self._recording = True
            self.report["stage"] = "execution"
            self.camera.begin_segment()
            function(self.camera, self)
            if self._failures:
                raise self._failures[0]
            if not self.report["steps"]:
                raise ValueError(
                    "A walkthrough must produce at least one illustrated step"
                )
        except BaseException as caught:
            error = caught
            self.report["error"] = f"{type(caught).__name__}: {caught}"
        finally:
            self._recording = False
            self._preparing = False
            try:
                self.camera.stop_recording()
            except BaseException as caught:
                if error is None:
                    error = caught
                    self.report["error"] = f"{type(caught).__name__}: {caught}"
            try:
                self.video.finish()
                self.report["video"] = "video.mp4"
            except BaseException as caught:
                if error is None:
                    error = caught
                    self.report["error"] = f"{type(caught).__name__}: {caught}"
            self.report["passed"] = error is None
            self.report["presentation"] = dict(
                fps=self.video.fps,
                idle_limit=self.video.idle_limit,
                skipped_guest_seconds=self.video.skipped_seconds,
                navigation_hold=self.navigation_hold,
                action_hold=self.action_hold,
                duration_seconds=self.video.seconds,
            )
            artifacts = {
                str(p.relative_to(self.output)): _digest(p)
                for p in self.output.rglob("*")
                if p.is_file() and p.name not in ("guide.json", ".guide.json.tmp")
            }
            self.report["artifacts"] = artifacts
            self.save()
        if error is not None:
            raise error
        return self.report
