"""Coverage summaries must preserve denominators and distinguish missing reports."""

import json
from pathlib import Path

from coverage_summary import summarize


def test_summary_keeps_zero_coverage_and_sums_lcov_records(tmp_path: Path) -> None:
    report = tmp_path / "native.lcov"
    report.write_text(
        "LF:4\nLH:0\nBRF:2\nBRH:0\nend_of_record\nLF:6\nLH:5\nBRF:2\nBRH:1\n"
    )
    summary = summarize(tmp_path)
    assert "50.00% (5/10)" in summary
    assert "25.00% (1/4)" in summary


def test_missing_report_is_not_presented_as_complete_coverage(tmp_path: Path) -> None:
    assert "not produced" in summarize(tmp_path)
    (tmp_path / "empty.lcov").write_text("LF:0\nLH:0\n")
    assert "— (0/0)" in summarize(tmp_path)


def test_python_branch_denominator_is_preserved(tmp_path: Path) -> None:
    directory = tmp_path / "python"
    directory.mkdir()
    (directory / "coverage.xml").write_text(
        '<coverage lines-covered="3" lines-valid="4" branches-covered="0" branches-valid="2"/>'
    )
    summary = summarize(tmp_path)
    assert "75.00% (3/4)" in summary
    assert "0.00% (0/2)" in summary


def test_web_json_is_reported_once_when_lcov_also_exists(tmp_path: Path) -> None:
    directory = tmp_path / "web"
    directory.mkdir()
    (directory / "coverage-summary.json").write_text(
        json.dumps({
            "total": {
                "lines": {"covered": 2, "total": 8},
                "branches": {"covered": 1, "total": 3},
            }
        })
    )
    (directory / "lcov.info").write_text("LF:8\nLH:2\nBRF:3\nBRH:1\n")
    summary = summarize(tmp_path)
    assert summary.count("25.00% (2/8)") == 1
    assert "33.33% (1/3)" in summary
