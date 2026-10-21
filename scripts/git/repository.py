"""Git access shared by the commit and sync tools."""

import subprocess
import time
from pathlib import Path

LOCK_WAIT_SECONDS = 120


class Failure(Exception):
    pass


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

    def names(self, *arguments: str) -> list[str]:
        return self.git(*arguments, "--name-only", "-z").stdout.split("\0")[:-1]

    def branch(self) -> str:
        result = self.git("symbolic-ref", "-q", "HEAD", check=False)
        if result.returncode:
            raise Failure("HEAD is detached; check out the branch to commit to")
        return result.stdout.strip()

    @staticmethod
    def toplevel(path: Path) -> Path:
        return Path(
            subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                cwd=path,
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip()
        )
