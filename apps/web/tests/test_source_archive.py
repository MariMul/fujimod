"""Lazy imports keep source inspection, namespaces and hash validation intact."""

import importlib
import importlib.util
import py_compile
import runpy
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

install = runpy.run_path(
    str(Path(__file__).parents[1] / "src/engine/source-archive.py")
)["install_source_archive"]


class SourceArchiveTests(unittest.TestCase):
    def test_imports_materialize_only_needed_modules_and_reject_stale_bytecode(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            repo = root / "repo"
            repo.mkdir()
            source = root / "module.py"
            source.write_text("value = 1\n")
            compiled = root / "module.pyc"
            py_compile.compile(
                str(source),
                cfile=str(compiled),
                doraise=True,
                invalidation_mode=py_compile.PycInvalidationMode.CHECKED_HASH,
            )
            archive = root / "sources.zip"
            with zipfile.ZipFile(archive, "w") as output:
                # Namespace package and deliberately mismatching source/PYC.
                output.writestr("source_fixture/needed.py", b"value = 2\n")
                output.writestr(
                    importlib.util.cache_from_source("source_fixture/needed.py"),
                    compiled.read_bytes(),
                )
                output.writestr(
                    "source_fixture/unused.py", b'raise AssertionError("Not imported")'
                )
                output.writestr("data.json", b"{}")
            finder = install(archive, repo, ("",))
            sys.path.insert(0, str(repo))
            try:
                self.assertTrue((repo / "data.json").is_file())
                self.assertFalse((repo / "source_fixture/needed.py").exists())
                module = importlib.import_module("source_fixture.needed")
                self.assertEqual(module.value, 2)
                assert module.__file__ is not None
                self.assertEqual(Path(module.__file__).read_bytes(), b"value = 2\n")
                self.assertFalse((repo / "source_fixture/unused.py").exists())
            finally:
                sys.meta_path.remove(finder)
                finder.archive.close()
                sys.path.remove(str(repo))
                sys.modules.pop("source_fixture.needed", None)
                sys.modules.pop("source_fixture", None)
                importlib.invalidate_caches()
