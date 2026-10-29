"""Check Rust workspace crates affected by commits pushed to the main branch."""

from __future__ import annotations

import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
from typing import Iterable


MAIN_REF = "refs/heads/main"
WASM_TARGET = "wasm32-unknown-unknown"
RUST_INPUT_NAMES = {
    "Cargo.lock", "Cargo.toml", "rust-toolchain", "rust-toolchain.toml",
}
WASM_MARKER = re.compile(
    r"wasm32|wasm-bindgen|web-sys|js-sys|target_family\s*=\s*['\"]wasm['\"]",
    re.IGNORECASE,
)


def parse_updates(lines: Iterable[str]) -> list[tuple[str, str, str, str]]:
    updates = []
    for line in lines:
        fields = line.split()
        if len(fields) != 4:
            raise ValueError(f"invalid pre-push update: {line.rstrip()}")
        updates.append(tuple(fields))
    return updates


def _git_output(root: Path, *arguments: str) -> bytes:
    result = subprocess.run(
        ["git", *arguments], cwd=root, capture_output=True, check=False,
    )
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(detail or f"git {' '.join(arguments)} failed")
    return result.stdout


def is_zero_oid(value: str) -> bool:
    return bool(value) and all(character == "0" for character in value)


def pushed_paths(root: Path, updates: Iterable[tuple[str, str, str, str]]) -> set[str]:
    paths = set()
    for _local_ref, local_sha, remote_ref, remote_sha in updates:
        if remote_ref != MAIN_REF or is_zero_oid(local_sha):
            continue
        revisions = ["rev-list", "--reverse", local_sha]
        if not is_zero_oid(remote_sha):
            revisions.append(f"^{remote_sha}")
        commits = _git_output(root, *revisions).decode("ascii").splitlines()
        for commit in commits:
            changed = _git_output(
                root, "diff-tree", "--root", "--no-commit-id", "-m",
                "--no-renames", "--name-only", "-r", "-z", commit,
            )
            paths.update(
                part.decode("utf-8", errors="surrogateescape")
                for part in changed.split(b"\0") if part
            )
    return paths


def is_rust_input(path: str) -> bool:
    candidate = PurePosixPath(path)
    return (
        candidate.suffix == ".rs"
        or candidate.name in RUST_INPUT_NAMES
        or bool(candidate.parts and candidate.parts[0] == ".cargo")
    )


def select_touched_crates(
    paths: Iterable[str], packages: Iterable[dict], workspace_root: Path,
) -> list[dict]:
    root = workspace_root.resolve()
    package_roots = []
    for package in packages:
        manifest = Path(package["manifest_path"]).resolve()
        try:
            relative_root = manifest.parent.relative_to(root)
        except ValueError:
            continue
        package_roots.append((relative_root, manifest.relative_to(root), package))

    selected = {}
    all_packages = {package["name"]: package for _, _, package in package_roots}
    for path in paths:
        if not is_rust_input(path):
            continue
        candidate = PurePosixPath(path)
        if (candidate in {
                PurePosixPath("Cargo.toml"), PurePosixPath("Cargo.lock"),
                PurePosixPath("rust-toolchain"), PurePosixPath("rust-toolchain.toml"),
        } or (candidate.parts and candidate.parts[0] == ".cargo")):
            selected.update(all_packages)
            continue

        matches = [
            (len(package_root.parts), package)
            for package_root, manifest, package in package_roots
            if candidate == manifest or candidate.is_relative_to(package_root)
        ]
        if matches:
            _, package = max(matches, key=lambda match: match[0])
            selected[package["name"]] = package
    return [selected[name] for name in sorted(selected)]


def package_supports_wasm(package: dict) -> bool:
    fujimod_metadata = (package.get("metadata") or {}).get("fujimod") or {}
    if fujimod_metadata.get("skip-wasm32-unknown-unknown-check") is True:
        return False

    package_root = Path(package["manifest_path"]).parent
    candidates = [package_root / "Cargo.toml", package_root / "README.md"]
    candidates.extend(package_root.rglob("*.rs"))
    for path in candidates:
        if any(part in {"target", ".git"} for part in path.parts):
            continue
        try:
            if WASM_MARKER.search(path.read_text(encoding="utf-8")):
                return True
        except (OSError, UnicodeDecodeError):
            continue
    return False


def workspace_packages(root: Path) -> list[dict]:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=root, capture_output=True, text=True, check=True,
    )
    return json.loads(result.stdout)["packages"]


def cargo_check_command(packages: Iterable[dict], target: str | None = None) -> list[str]:
    command = ["cargo", "check", "--locked"]
    if target:
        command.extend(("--target", target))
    for package in packages:
        command.extend(("--package", package["name"]))
    return command


def check_updates(root: Path, updates: Iterable[tuple[str, str, str, str]]) -> int:
    main_updates = [update for update in updates if update[2] == MAIN_REF]
    if not main_updates:
        return 0

    paths = pushed_paths(root, main_updates)
    if not any(is_rust_input(path) for path in paths):
        print("No Rust inputs changed; skipping cargo check.", flush=True)
        return 0

    packages = select_touched_crates(paths, workspace_packages(root), root)
    if not packages:
        print("No workspace crates were affected by the pushed Rust inputs.", flush=True)
        return 0

    names = ", ".join(package["name"] for package in packages)
    print(f"Checking native target for: {names}", flush=True)
    result = subprocess.run(cargo_check_command(packages), cwd=root, check=False)
    if result.returncode:
        return result.returncode

    wasm_packages = [package for package in packages if package_supports_wasm(package)]
    if wasm_packages:
        names = ", ".join(package["name"] for package in wasm_packages)
        print(f"Checking {WASM_TARGET} for: {names}", flush=True)
        result = subprocess.run(
            cargo_check_command(wasm_packages, WASM_TARGET), cwd=root, check=False,
        )
        if result.returncode:
            return result.returncode
    return 0


def main() -> int:
    try:
        root_result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, check=True,
        )
        root = Path(root_result.stdout.strip())
        return check_updates(root, parse_updates(sys.stdin))
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"Rust push check failed: {error}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
