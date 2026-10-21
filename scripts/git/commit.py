"""Commit chosen paths or hunks from a shared checkout and push, without a worktree.

    mise run commit -- -m "Describe the change" path/one path/two
    mise run commit -- -m "Describe the change" --patch my-hunks.diff [path ...]

Whole paths take their current working-tree content (additions, edits and
deletions). ``--patch`` applies a diff, relative to HEAD, holding only your
hunks in files that also contain other work. Nothing else in the shared index
or working tree is included.

The commit is built in a private index on the checked-out branch's HEAD, after
fast-forwarding that branch to its ``origin`` counterpart. The branch moves only
if it is still at that HEAD, then the commit is pushed (fast-forward only). If
origin moved meanwhile, the commit is rebuilt on the new head and retried.
Afterward each committed path's shared index entry is updated when nobody else
staged that path.
"""

import argparse
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ATTEMPTS = 5
LOCK_WAIT_SECONDS = 120


class Failure(Exception):
    pass


class Arguments(argparse.Namespace):
    message: str
    patch: Path | None
    repo: Path
    paths: list[str]


class Repository:
    def __init__(self, root: Path) -> None:
        self.root = root

    def git(
        self,
        *arguments: str,
        env: dict[str, str] | None = None,
        input: str | None = None,
        check: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        deadline = time.monotonic() + LOCK_WAIT_SECONDS
        while True:
            result = subprocess.run(
                ["git", *arguments],
                cwd=self.root,
                env=env,
                input=input,
                capture_output=True,
                text=True,
                check=False,
            )
            # Another Git process in the shared checkout holds the index briefly.
            if "index.lock': File exists" not in result.stderr:
                break
            if time.monotonic() > deadline:
                break
            time.sleep(0.5)
        if check and result.returncode:
            command = " ".join(arguments)
            raise Failure(f"git {command} failed:\n{result.stderr.strip()}")
        return result

    def output(self, *arguments: str) -> str:
        return self.git(*arguments).stdout.strip()

    def branch(self) -> str:
        result = self.git("symbolic-ref", "-q", "HEAD", check=False)
        if result.returncode:
            raise Failure("HEAD is detached; check out the branch to commit to")
        return result.stdout.strip()

    def fast_forward(self, name: str) -> None:
        """Bring the checkout to origin's branch; Git refuses if it would touch local edits."""
        self.git("fetch", "-q", "origin", name)
        remote = self.output("rev-parse", "FETCH_HEAD")
        current = self.output("rev-parse", "HEAD")
        if remote == current:
            return
        if (
            self.git(
                "merge-base", "--is-ancestor", current, remote, check=False
            ).returncode
            == 0
        ):
            self.git("merge", "--ff-only", "-q", remote)
        elif self.git(
            "merge-base", "--is-ancestor", remote, current, check=False
        ).returncode:
            raise Failure(
                f"local {name} {current[:10]} and origin {remote[:10]} have diverged; "
                "resolve that before committing"
            )

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

    def names(self, *arguments: str) -> list[str]:
        return self.git(*arguments, "--name-only", "-z").stdout.split("\0")[:-1]

    def reconcile_index(self, parent: str, commit: str) -> list[str]:
        """Point shared index entries at the new commit where they still equal the old HEAD."""
        names = self.names("diff", parent, commit)
        staged = set(self.names("diff", "--cached", parent))
        clean = [name for name in names if name not in staged]
        if clean:
            self.git(
                "reset",
                "-q",
                commit,
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
                input="\0".join(clean) + "\0",
            )
        return sorted(staged.intersection(names))

    def commit(
        self, paths: list[str], patch: Path | None, message: str
    ) -> tuple[str, list[str]]:
        ref = self.branch()
        name = ref.removeprefix("refs/heads/")
        for _ in range(ATTEMPTS):
            self.fast_forward(name)
            parent = self.output("rev-parse", "HEAD")
            result = self.build(parent, paths, patch, message)
            # Moves the branch only if no other commit landed since HEAD was read.
            if self.git("update-ref", ref, result, parent, check=False).returncode:
                continue
            pushed = self.git("push", "-q", "origin", f"{result}:{ref}", check=False)
            if pushed.returncode == 0:
                try:
                    return result, self.reconcile_index(parent, result)
                except Failure as error:
                    # The commit is pushed; only the shared index is stale for its paths.
                    print(
                        f"commit: pushed {result[:10]}, but updating the shared index "
                        f"failed; run `git reset -q HEAD -- PATHS` for its paths:\n{error}",
                        file=sys.stderr,
                    )
                    return result, []
            # Origin moved: step the branch back unless something else moved it, then rebuild.
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
        root = Path(
            subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                cwd=args.repo,
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip()
        )
        repository = Repository(root)
        result, skipped = repository.commit(args.paths, patch, message)
    except (Failure, subprocess.CalledProcessError) as error:
        print(f"commit: {error}", file=sys.stderr)
        sys.exit(1)
    print(repository.git("show", "--stat", "--format=%h %s", result).stdout, end="")
    if skipped:
        print(
            "Shared index left unchanged for paths someone else staged: "
            + ", ".join(skipped)
        )


if __name__ == "__main__":
    main()
