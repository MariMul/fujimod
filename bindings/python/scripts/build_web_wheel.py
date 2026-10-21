#!/usr/bin/env python3
"""Build the browser-only wheel from the same Python package as native maturin.

Requires Python 3.11+. The JavaScript/Wasm package is loaded separately by the
host page and registered with registerPyodide(); this wheel contains no USB
implementation or native binary.
"""

import argparse
import base64
import csv
import hashlib
import io
import json
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def build(output: Path) -> Path:
    project = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]
    version = project["version"]
    dist = f"fujimod-{version}.dist-info"
    package = ROOT / "python" / "fujimod"
    files = {
        str(p.relative_to(ROOT / "python")): p.read_bytes()
        for p in sorted(package.rglob("*.py"))
        if p.relative_to(package).parts[0] != "emulator"
    }
    files[f"{dist}/METADATA"] = (
        f"Metadata-Version: 2.4\nName: fujimod\nVersion: {version}\n"
        f"Summary: FujiMod WebUSB Python adapter for Pyodide\nRequires-Python: {project['requires-python']}\nLicense-File: NOTICE\n\n"
    ).encode()
    files[f"{dist}/WHEEL"] = (
        b"Wheel-Version: 1.0\nGenerator: fujimod-web\nRoot-Is-Purelib: true\nTag: py3-none-pyemscripten_2026_0_wasm32\n"
    )
    files[f"{dist}/licenses/NOTICE"] = (ROOT / "NOTICE").read_bytes()
    rows = []
    for path, data in files.items():
        digest = (
            base64
            .urlsafe_b64encode(hashlib.sha256(data).digest())
            .rstrip(b"=")
            .decode()
        )
        rows.append((path, "sha256=" + digest, str(len(data))))
    rows.append((f"{dist}/RECORD", "", ""))
    record = io.StringIO(newline="")
    csv.writer(record).writerows(rows)
    files[f"{dist}/RECORD"] = record.getvalue().encode()
    output.mkdir(parents=True, exist_ok=True)
    path = output / f"fujimod-{version}-py3-none-pyemscripten_2026_0_wasm32.whl"
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as wheel:
        for name, data in files.items():
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            wheel.writestr(info, data)
    (output / "manifest.json").write_text(json.dumps({"wheel": path.name}) + "\n")
    print(path)
    return path


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out", type=Path, default=ROOT.parents[1] / "target" / "web-wheels"
    )
    build(parser.parse_args().out)
