"""Render independently measured coverage into the GitHub Actions job summary."""

from __future__ import annotations

import json
import xml.etree.ElementTree as ET
from pathlib import Path


def percentage(hit: int, found: int) -> str:
    return f"{100 * hit / found:.2f}% ({hit}/{found})" if found else "— (0/0)"


def lcov_counts(path: Path) -> dict[str, int]:
    counts = dict.fromkeys(("LF", "LH", "BRF", "BRH"), 0)
    for line in path.read_text().splitlines():
        key, _, value = line.partition(":")
        if key in counts:
            counts[key] += int(value)
    return counts


def summarize(root: Path) -> str:
    rows: list[tuple[str, str, str]] = []
    python = root / "python/coverage.xml"
    if python.is_file():
        report = ET.parse(python).getroot()
        rows.append((
            "Python (native test suite)",
            percentage(
                int(report.attrib["lines-covered"]), int(report.attrib["lines-valid"])
            ),
            percentage(
                int(report.attrib["branches-covered"]),
                int(report.attrib["branches-valid"]),
            ),
        ))
    web = root / "web/coverage-summary.json"
    if web.is_file():
        total = json.loads(web.read_text())["total"]
        rows.append((
            "Web (Vitest)",
            percentage(total["lines"]["covered"], total["lines"]["total"]),
            percentage(total["branches"]["covered"], total["branches"]["total"]),
        ))
    for path in sorted(root.rglob("*.lcov")) + sorted(root.rglob("*.info")):
        if path.parent.name == "web":
            continue  # Vitest already supplies a summary of this same measurement.
        counts = lcov_counts(path)
        rows.append((
            str(path.relative_to(root)),
            percentage(counts["LH"], counts["LF"]),
            percentage(counts["BRH"], counts["BRF"]),
        ))
    if not rows:
        return (
            "Coverage reports were not produced. Check the test/setup failures above.\n"
        )
    lines = [
        "| Suite | Lines | Branches |",
        "| --- | --- | --- |",
        *(f"| {suite} | {line} | {branch} |" for suite, line, branch in rows),
        "",
        "Download this job's coverage artifact for HTML and machine-readable reports.",
        "Node coverage measures host JavaScript; Rust/Wasm, browser and Worker isolates are separate execution environments.",
        "Python's native-suite report includes unexecuted Pyodide adapters as uncovered; Pyodide contract tests run separately.",
        "",
    ]
    return "\n".join(lines)


if __name__ == "__main__":
    print(summarize(Path("target/coverage")))
