"""Build the Python extension and install it only after it loads.

`maturin develop` overwrites the package's `_native.abi3.so` in place, so a
failed, partial or reduced-feature build breaks every process that shares the
checkout. This builds a wheel in a temporary directory, loads its extension in
a fresh interpreter, requires every default-feature module, and then renames it
over the installed file. A build that fails any step leaves the installed
extension untouched. The running environment gets the same `fujimod.pth`
source-path file an editable install writes.

    uv run --locked python scripts/python_extension.py
"""

import os
import shutil
import site
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "bindings/python/python/fujimod"
NAME = "_native.abi3.so"
MODULES = ("catalog", "corpus", "emulator", "firmware", "recording", "remote", "usb")
CHECK = """
import importlib.util, sys
spec = importlib.util.spec_from_file_location('fujimod._native', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
missing = [name for name in sys.argv[2:] if not hasattr(module, name)]
if missing:
    raise SystemExit('extension lacks modules: ' + ', '.join(missing))
"""


def main() -> None:
    maturin = shutil.which("maturin")
    if maturin is None:
        raise SystemExit("maturin is not installed; run through `uv run --locked`")
    with tempfile.TemporaryDirectory(
        prefix="python-extension-", dir=ROOT / "target"
    ) as work:
        subprocess.run(
            [
                maturin,
                "build",
                "--profile",
                "iterate",
                "--manifest-path",
                "bindings/python/Cargo.toml",
                "--features",
                "testing",
                "--locked",
                "--out",
                work,
            ],
            cwd=ROOT,
            check=True,
        )
        wheels = list(Path(work).glob("*.whl"))
        if len(wheels) != 1:
            raise SystemExit(f"expected one wheel, found {len(wheels)}")
        candidate = PACKAGE / f"._native.new-{os.getpid()}.abi3.so"
        try:
            with zipfile.ZipFile(wheels[0]) as wheel, candidate.open("wb") as out:
                out.write(wheel.read("fujimod/" + NAME))
            candidate.chmod(0o755)
            subprocess.run(
                [sys.executable, "-c", CHECK, str(candidate), *MODULES], check=True
            )
            os.replace(candidate, PACKAGE / NAME)
        finally:
            candidate.unlink(missing_ok=True)
    (Path(site.getsitepackages()[0]) / "fujimod.pth").write_text(
        str(PACKAGE.parent) + "\n"
    )
    print(f"Installed {PACKAGE / NAME}")


if __name__ == "__main__":
    main()
