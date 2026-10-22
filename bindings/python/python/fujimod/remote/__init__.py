"""Reference-based camera control over local Rust futures."""
from __future__ import annotations

import asyncio
from dataclasses import asdict, dataclass
import json
from typing import Callable

from .._native import remote as _native
from .. import recording


@dataclass(frozen=True)
class Menu:
    id: str
    key: int | None = None
    via: Menu | None = None


@dataclass(frozen=True)
class Option:
    item: Menu
    name: str


@dataclass(frozen=True)
class Control:
    id: str


Reference = Menu | Option | Control


def facts_from_selection(selection: dict, manifest: dict | None = None) -> dict:
    return json.loads(_native.facts_from_selection(json.dumps(selection), json.dumps(manifest)))


def menu(id: str | int, *, key: int | None = None, via: Menu | None = None) -> Menu:
    return Menu(str(id), key, via)


def option(item: Menu, name: str) -> Option:
    return Option(item, name)


def control(id: str) -> Control:
    return Control(id)


def _reference(value: Reference | None) -> str:
    if value is None:
        return "null"
    kind = {Menu: "menu", Option: "option", Control: "control"}[type(value)]
    return json.dumps({"kind": kind, "reference": value.id if isinstance(value, Control) else asdict(value)})


@dataclass(frozen=True)
class State:
    mode: int
    menu_active: bool
    node: int
    cursor: int
    current_key: int | None
    ready: bool
    pending: list[str]


@dataclass(frozen=True)
class MenuState:
    node: int
    key: int | None
    index: int | None
    child: int | None
    label: str
    exists: bool
    visible: bool
    selected: bool
    enabled: bool
    value: str | None
    ancestors: list[dict]
    root: str


@dataclass(frozen=True)
class ControlState:
    id: str
    position: str | None
    supported: bool


@dataclass(frozen=True)
class Snapshot:
    state: State
    value: MenuState | ControlState
    reads: dict


@dataclass(frozen=True)
class Capture:
    frame: recording.Frame
    time: int
    clock: str
    output: str
    sequence: int


def _convert(value):
    if isinstance(value, str):
        data = json.loads(value)
        kind = MenuState if data["value"]["kind"] == "menu" else ControlState
        return Snapshot(State(**data["state"]), kind(**data["value"]["value"]), data["reads"])
    if isinstance(value, tuple):
        png, time, clock, output, sequence = value
        return Capture(recording.Frame.from_png(png), time, clock, output, sequence)
    return value


async def _drive(task):
    loop = asyncio.get_running_loop()
    try:
        while True:
            wake = loop.create_future()

            def resumed(future=wake):
                def notify():
                    if not future.done():
                        future.set_result(None)
                if not loop.is_closed():
                    loop.call_soon_threadsafe(notify)

            if task.poll(resumed):
                return _convert(task.result())
            await wake
            await asyncio.sleep(0)
    finally:
        task.cancel()


class Camera:
    def __init__(self, native):
        self._native = native

    @classmethod
    def from_board(cls, board, facts: dict, *, quantum: int, window_instructions: int,
                   scanout_node: str, frame_counter_kind: str, frame_counter_pointer: str,
                   output: str = "lcd") -> Camera:
        options = dict(quantum=quantum, window_instructions=window_instructions,
                       scanout_node=scanout_node, output=output,
                       frame_counter_kind=frame_counter_kind,
                       frame_counter_pointer=frame_counter_pointer)
        return cls(_native.Camera.from_board(board, json.dumps(facts), json.dumps(options)))

    @classmethod
    def from_ff80(cls, session, facts: dict, *, read_bytes: int, write_bytes: int,
                  event_bytes: int, poll_seconds: float = .1, output: str = "lcd") -> Camera:
        return cls(_native.Camera.from_ff80(session, json.dumps(facts), read_bytes, write_bytes,
                                            event_bytes, poll_seconds, output))

    @property
    def now(self) -> int:
        return self._native.now

    @property
    def blocking(self) -> BlockingCamera:
        return BlockingCamera(self)

    def _task(self, action: str, reference: Reference | None, deadline: int, steps: int = 0):
        return self._native.operation(action, _reference(reference), deadline, steps)

    async def read(self, reference: Reference) -> Snapshot:
        return await _drive(self._task("read", reference, 0))

    async def navigate(self, reference: Menu, *, deadline: int) -> Snapshot:
        return await _drive(self._task("navigate", reference, deadline))

    async def select(self, reference: Option, *, deadline: int) -> None:
        await _drive(self._task("select", reference, deadline))

    async def press(self, reference: Control, *, deadline: int) -> None:
        await _drive(self._task("press", reference, deadline))

    async def turn(self, reference: Control, n: int, *, deadline: int) -> None:
        await _drive(self._task("turn", reference, deadline, n))

    async def frame(self) -> Capture:
        return await _drive(self._task("frame", None, 0))

    async def screenshot(self, *, deadline: int) -> Capture:
        return await _drive(self._task("screenshot", None, deadline))

    async def wait_until(self, predicate: Callable[[Snapshot], bool], reference: Reference,
                         *, deadline: int) -> Snapshot:
        return await _drive(self._native.wait_until(_reference(reference), predicate, deadline))

    def close(self) -> None:
        self._native.close()


class BlockingCamera:
    def __init__(self, camera: Camera):
        self._camera = camera

    @property
    def now(self) -> int:
        return self._camera.now

    def _call(self, action: str, reference: Reference | None = None, deadline: int = 0, steps: int = 0):
        return _convert(self._camera._task(action, reference, deadline, steps).blocking())

    def read(self, reference: Reference) -> Snapshot:
        return self._call("read", reference)

    def navigate(self, reference: Menu, *, deadline: int) -> Snapshot:
        return self._call("navigate", reference, deadline)

    def select(self, reference: Option, *, deadline: int) -> None:
        self._call("select", reference, deadline)

    def press(self, reference: Control, *, deadline: int) -> None:
        self._call("press", reference, deadline)

    def turn(self, reference: Control, n: int, *, deadline: int) -> None:
        self._call("turn", reference, deadline, n)

    def frame(self) -> Capture:
        return self._call("frame")

    def screenshot(self, *, deadline: int) -> Capture:
        return self._call("screenshot", deadline=deadline)

    def wait_until(self, predicate: Callable[[Snapshot], bool], reference: Reference,
                   *, deadline: int) -> Snapshot:
        return _convert(self._camera._native.wait_until(_reference(reference), predicate, deadline).blocking())
