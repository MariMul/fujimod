"""Materialize source and checked bytecode only when the import system needs it.

Imported modules retain real files and ordinary SourceFileLoader semantics.
Repository-relative data stays separate from the compressed Python archive.
"""

import importlib.abc
import importlib.util
import sys
import zipfile
from collections.abc import Sequence
from pathlib import Path, PurePosixPath
from types import ModuleType


class SourceArchive(importlib.abc.MetaPathFinder):
    def __init__(
        self, archive: str | Path, root: str | Path, prefixes: Sequence[str]
    ) -> None:
        self.archive = zipfile.ZipFile(archive)
        self.root = Path(root)
        self.prefixes = tuple(prefixes)
        self.names = set(self.archive.namelist())
        self.directories = set()
        for name in self.names:
            relative = PurePosixPath(name)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("Source archive path escapes the repository")
            self.directories.update(str(parent) for parent in relative.parents)
            if not name.endswith((".py", ".pyc", "/")):
                self.extract(name)

    def extract(self, name: str) -> None:
        target = self.root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(self.archive.read(name))

    def find_spec(
        self,
        fullname: str,
        path: Sequence[str] | None = None,
        target: ModuleType | None = None,
    ) -> None:
        module = fullname.replace(".", "/")
        for prefix in self.prefixes:
            base = prefix + module
            for name in (base + "/__init__.py", base + ".py"):
                if name not in self.names:
                    continue
                self.extract(name)
                cached = importlib.util.cache_from_source(name)
                if cached in self.names:
                    self.extract(cached)
                importlib.invalidate_caches()
                # Delegate to the usual file loader, including checked hashes.
                return None
            if base in self.directories:
                (self.root / base).mkdir(parents=True, exist_ok=True)
                importlib.invalidate_caches()
        return None


def install_source_archive(
    archive: str | Path, root: str | Path, prefixes: Sequence[str]
) -> SourceArchive:
    finder = SourceArchive(archive, root, prefixes)
    sys.meta_path.insert(0, finder)
    return finder
