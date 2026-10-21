import subprocess
from pathlib import Path

import pytest
from commit import Failure, Repository


def run(*arguments: str, cwd: Path) -> str:
    return subprocess.run(
        arguments, cwd=cwd, check=True, capture_output=True, text=True
    ).stdout


def clone(remote: Path, path: Path) -> Path:
    run("git", "clone", "-q", str(remote), str(path), cwd=path.parent)
    for name in ("user.name", "user.email"):
        run("git", "config", name, "Test", cwd=path)
    return path


@pytest.fixture
def repos(tmp_path: Path) -> tuple[Path, Path]:
    remote = tmp_path / "remote.git"
    run("git", "init", "-q", "--bare", "-b", "main", str(remote), cwd=tmp_path)
    checkout = clone(remote, tmp_path / "checkout")
    (checkout / "a.txt").write_text("one\ntwo\nthree\n")
    (checkout / "b.txt").write_text("b\n")
    run("git", "add", ".", cwd=checkout)
    run("git", "commit", "-qm", "base", cwd=checkout)
    run("git", "push", "-q", "origin", "main", cwd=checkout)
    return remote, checkout


def test_commit_takes_only_named_paths_and_pushes(repos: tuple[Path, Path]) -> None:
    remote, checkout = repos
    (checkout / "a.txt").write_text("one\ntwo\nthree\nfour\n")
    (checkout / "b.txt").write_text("someone else\n")
    run("git", "add", "b.txt", cwd=checkout)
    result, skipped = Repository(checkout).commit(["a.txt"], None, "Add four\n")
    assert run("git", "rev-parse", "main", cwd=remote).strip() == result
    files = run("git", "show", "--name-only", "--format=", result, cwd=checkout).split()
    assert files == ["a.txt"]
    assert run("git", "status", "--porcelain", cwd=checkout).splitlines() == [
        "M  b.txt"
    ]
    assert skipped == []


def test_patch_commits_chosen_hunks_and_keeps_the_rest(
    repos: tuple[Path, Path], tmp_path: Path
) -> None:
    _, checkout = repos
    (checkout / "a.txt").write_text("ONE\ntwo\nthree\n")
    patch = tmp_path / "mine.diff"
    patch.write_text(run("git", "diff", cwd=checkout))
    (checkout / "a.txt").write_text("ONE\ntwo\nthree\nother work\n")
    result, _ = Repository(checkout).commit([], patch, "Capitalize\n")
    assert run("git", "show", f"{result}:a.txt", cwd=checkout) == "ONE\ntwo\nthree\n"
    assert (checkout / "a.txt").read_text() == "ONE\ntwo\nthree\nother work\n"
    assert run("git", "diff", cwd=checkout).count("+other work") == 1


def test_commit_rebuilds_on_a_newer_origin(
    repos: tuple[Path, Path], tmp_path: Path
) -> None:
    remote, checkout = repos
    other = clone(remote, tmp_path / "other")
    (other / "c.txt").write_text("c\n")
    run("git", "add", "c.txt", cwd=other)
    run("git", "commit", "-qm", "other", cwd=other)
    run("git", "push", "-q", "origin", "main", cwd=other)
    (checkout / "b.txt").write_text("mine\n")
    result, _ = Repository(checkout).commit(["b.txt"], None, "Mine\n")
    parent = run("git", "rev-parse", f"{result}^", cwd=checkout).strip()
    assert parent == run("git", "rev-parse", "main", cwd=other).strip()
    assert (checkout / "c.txt").read_text() == "c\n"


def test_nothing_to_commit_fails(repos: tuple[Path, Path]) -> None:
    _, checkout = repos
    with pytest.raises(Failure, match="nothing to commit"):
        Repository(checkout).commit(["a.txt"], None, "Nothing\n")


def test_detached_head_is_refused(repos: tuple[Path, Path]) -> None:
    _, checkout = repos
    run("git", "checkout", "-q", "--detach", cwd=checkout)
    (checkout / "a.txt").write_text("detached\n")
    with pytest.raises(Failure, match="detached"):
        Repository(checkout).commit(["a.txt"], None, "Detached\n")
