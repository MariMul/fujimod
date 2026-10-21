"""Host client for a resolved definition on the shared Rust Board/Session."""

from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Self, TypeGuard

from fujimod._native import emulator as _native

from .definition import Definition, Xp4Definition
from .processor import Processor


def _is_json_object(value: object) -> TypeGuard[dict[str, object]]:
    return isinstance(value, dict) and all(isinstance(key, str) for key in value)


def _json_object(value: object) -> dict[str, object]:
    if not _is_json_object(value):
        raise TypeError("Native JSON result must be a string-keyed object")
    return value


class Board:
    def __init__(
        self, definition: Definition | Xp4Definition, assets: Mapping[str, bytes]
    ) -> None:
        if type(definition.quantum) is not int or definition.quantum < 1:
            raise ValueError("Board quantum must be a positive integer")
        loaded = dict(assets)
        _native.verify_input_assets(
            json.dumps([asset.manifest() for asset in definition.assets]), loaded
        )
        images = []
        for image in definition.images:
            if image.asset not in loaded:
                raise ValueError("Image names an undeclared asset: " + image.asset)
            images.append((image, loaded[image.asset]))
        self.quantum = definition.quantum
        if isinstance(definition, Xp4Definition):
            if any(image.core is not None for image, _ in images):
                raise ValueError("XP4 input images require physical addresses")
            if definition.storage is not None and definition.storage not in loaded:
                raise ValueError(
                    "Storage names an undeclared asset: " + definition.storage
                )
            self._native = _native.Board.from_xp4(
                str(definition.library),
                json.dumps(definition.options),
                storage_image=None
                if definition.storage is None
                else loaded[definition.storage],
            )
        else:
            self._native = _native.Board(
                definition.library,
                json.dumps(definition.execution),
                json.dumps(definition.devices),
                json.dumps(definition.scene),
                loaded,
                dict(definition.adapters),
                execution_mode=definition.mode,
                continue_refusals=definition.continue_refusals,
                engine=definition.engine,
                translation_cache_bytes=definition.translation_cache_bytes,
            )
        try:
            for image, data in images:
                if image.core is None:
                    self._native.physical_write(image.address, data)
                else:
                    self._native.mem_write(image.core, image.address, data)
        except BaseException:
            self._native.close()
            raise
        self.cpus = tuple(
            Processor(self, core) for core in range(len(self._native.instructions))
        )

    @property
    def native(self) -> _native.Board:
        return self._native

    @property
    def instructions(self) -> int:
        return sum(self._native.instructions)

    @property
    def virtual_ticks(self) -> int:
        return self._native.now

    @property
    def quiescent(self) -> bool:
        return self._native.quiescent

    @property
    def failure(self) -> dict[str, object] | None:
        failure = self.state()["failure"]
        return None if failure is None else _json_object(failure)

    def run(
        self,
        limit: int = 100000,
        *,
        pause_after: int | None = None,
        pause_at_tick: int | None = None,
        store_watches: list[tuple[int, int]] | None = None,
        device_watches: list[str] | None = None,
    ) -> None:
        # Stores name half-open canonical physical ranges. Device names are
        # nodes in the resolved graph. Watches last only for this host call.
        self._native.run(limit, self.quantum, pause_after, pause_at_tick, store_watches, device_watches)

    def run_until(self, tick: int) -> None:
        self._native.run_until(tick, self.quantum)

    def read_memory(self, address: int, size: int, cpu_id: int = 0) -> bytes:
        return self._native.mem_read(cpu_id, address, size)

    def write_memory(self, address: int, data: bytes, cpu_id: int = 0) -> None:
        self._native.mem_write(cpu_id, address, data)

    def read_physical(self, address: int, size: int) -> bytes:
        return self._native.physical_read(address, size)

    def write_physical(self, address: int, data: bytes) -> None:
        self._native.physical_write(address, data)

    def state(self) -> dict[str, object]:
        return _json_object(json.loads(self._native.state()))

    def fingerprint(self) -> dict[str, object]:
        return _json_object(json.loads(self._native.fingerprint()))

    def checkpoint(
        self, previous: _native.Checkpoint | None = None
    ) -> _native.Checkpoint:
        return self._native.checkpoint(previous)

    def restore_checkpoint(self, saved: _native.Checkpoint) -> None:
        self._native.restore_checkpoint(saved)

    def close(self) -> None:
        self._native.close()

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()


def make_board(
    definition: Definition | Xp4Definition, assets: Mapping[str, bytes]
) -> Board:
    return Board(definition, assets)
