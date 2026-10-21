"""Resolved execution facts and content identities supplied by a caller."""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal


@dataclass(frozen=True)
class Asset:
    id: str
    size: int
    sha256: str

    def manifest(self) -> dict[str, object]:
        return dict(id=self.id, content=dict(size=self.size, sha256=self.sha256))


@dataclass(frozen=True)
class Image:
    asset: str
    address: int
    core: int | None = None


@dataclass(frozen=True)
class Definition:
    execution: dict[str, object]
    devices: dict[str, object]
    assets: tuple[Asset, ...]
    images: tuple[Image, ...] = ()
    scene: dict[str, object] = field(default_factory=dict)
    library: str | Path | None = None
    engine: Literal["unicorn", "hypervisor"] = "unicorn"
    quantum: int = 1000
    translation_cache_bytes: int = 16 << 20
    mode: Literal["strict", "discovery"] = "strict"
    continue_refusals: bool = False
    adapters: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class Xp4Definition:
    options: dict[str, object]
    library: str | Path
    assets: tuple[Asset, ...] = ()
    images: tuple[Image, ...] = ()
    storage: str | None = None
    quantum: int = 1000
