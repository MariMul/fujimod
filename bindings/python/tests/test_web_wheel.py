"""Package contents for the browser-only wheel."""

import importlib.util
import zipfile
from pathlib import Path


def test_browser_wheel_keeps_only_supported_python_namespaces(tmp_path: Path) -> None:
    script = Path(__file__).parents[1] / "scripts" / "build_web_wheel.py"
    spec = importlib.util.spec_from_file_location("web_wheel_builder", script)
    assert spec is not None and spec.loader is not None
    builder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(builder)

    wheel = builder.build(tmp_path)
    with zipfile.ZipFile(wheel) as archive:
        names = set(archive.namelist())
    assert "fujimod/__init__.py" in names
    assert "fujimod/usb/__init__.py" in names
    assert not any(name.startswith("fujimod/emulator/") for name in names)
