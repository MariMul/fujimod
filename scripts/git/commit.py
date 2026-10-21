"""Commit chosen paths or hunks from a shared checkout and push, without a worktree.

    mise run commit -- -m "Describe the change" path/one path/two
    mise run commit -- -m "Describe the change" --patch my-hunks.diff [path ...]

Whole paths take their current working-tree content (additions, edits and
deletions). ``--patch`` applies a diff, relative to HEAD, holding only your
hunks in files that also contain other work. Nothing else in the shared index
or working tree is included.

The commit is built in a private index on the checked-out branch's HEAD, after
``sync.py`` brings that branch to its ``origin`` counterpart, merging origin's
changes into anyone's edits. Paths recorded as sync conflicts are refused. One
lock serializes syncs and commits in the checkout: the branch moves, the commit
is pushed (fast-forward only) and the shared index is reset to the new HEAD
before the next one starts. If origin moved meanwhile, the commit is rebuilt
on the new head and retried. Files on disk are never changed by committing.
"""

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from repository import Failure
from repository import Repository as GitRepository
from sync import load_conflicts, locked, update

ATTEMPTS = 5


class Arguments(argparse.Namespace):
    message: str
    patch: Path | None
    repo: Path
    paths: list[str]


class Repository(GitRepository):
    def build(
        self, parent: str, paths: list[str], patch: Path | None, message: str
    ) -> str:
        with tempfile.TemporaryDirectory() as scratch:
            env = dict(os.environ, GIT_INDEX_FILE=str(Path(scratch) / "index"))
            self.git("read-tree", parent, env=env)
            if patch:
                self.git(
                    "apply", "--cached", "--whitespace=nowarn", str(patch), env=env
                )
            if paths:
                self.git(
                    "add",
                    "-A",
                    "--pathspec-from-file=-",
                    "--pathspec-file-nul",
                    env=env,
                    input="\0".join(paths) + "\0",
                )
            tree = self.git("write-tree", env=env).stdout.strip()
        if tree == self.output("rev-parse", f"{parent}^{{tree}}"):
            raise Failure("nothing to commit: the chosen paths and hunks match HEAD")
        return self.git("commit-tree", tree, "-p", parent, input=message).stdout.strip()

    def touched(self, paths: list[str], patch: Path | None) -> set[str]:
        names = set(paths)
        if patch:
            numstat = self.output("apply", "--numstat", "-z", str(patch))
            # Each record is "added<TAB>deleted<TAB>path".
            names.update(
                record.split("\t")[-1]
                for record in numstat.split("\0")
                if "\t" in record
            )
        return names

    def commit(self, paths: list[str], patch: Path | None, message: str) -> str:
        ref = self.branch()
        name = ref.removeprefix("refs/heads/")
        conflicted = sorted(
            set(load_conflicts(self)).intersection(self.touched(paths, patch))
        )
        if conflicted:
            raise Failure(
                "origin changed these paths under your edits; merge them by hand, then "
                "run `python fujimod/scripts/git/sync.py --resolved PATH`: "
                + ", ".join(conflicted)
            )
        with locked(self):
            for _ in range(ATTEMPTS):
                update(self)
                parent = self.output("rev-parse", "HEAD")
                result = self.build(parent, paths, patch, message)
                if self.git("update-ref", ref, result, parent, check=False).returncode:
                    raise Failure(
                        f"{name} moved outside the commit tool during a commit"
                    )
                pushed = self.git(
                    "push", "-q", "origin", f"{result}:{ref}", check=False
                )
                if pushed.returncode == 0:
                    self.reset_index()
                    return result
                # Origin moved: step the branch back, then sync and rebuild.
                if self.git("update-ref", ref, parent, result, check=False).returncode:
                    raise Failure(
                        f"push failed and {name} moved locally; {result} is committed "
                        f"but not pushed:\n{pushed.stderr.strip()}"
                    )
        raise Failure(f"{name} kept moving; gave up after {ATTEMPTS} attempts")


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("-m", "--message", required=True)
    parser.add_argument(
        "--patch", type=Path, help="diff relative to HEAD holding chosen hunks"
    )
    parser.add_argument(
        "--repo", type=Path, default=Path.cwd(), help="checkout to commit in"
    )
    parser.add_argument("paths", nargs="*")
    args = parser.parse_args(namespace=Arguments())
    if not args.paths and not args.patch:
        parser.error("name the paths to commit or give --patch")
    message = args.message if args.message.endswith("\n") else args.message + "\n"
    patch = args.patch.resolve() if args.patch else None
    try:
        repository = Repository(GitRepository.toplevel(args.repo))
        result = repository.commit(args.paths, patch, message)
    except (Failure, subprocess.CalledProcessError) as error:
        print(f"commit: {error}", file=sys.stderr)
        sys.exit(1)
    print(repository.git("show", "--stat", "--format=%h %s", result).stdout, end="")


if __name__ == "__main__":
    main()
