#!/usr/bin/env python3
"""Wrapper around `ruff format` that notifies when fixes were applied."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from utils import file_hash

MESSAGE = (
    "We've found a formatting issue, but it was automatically repaired by the Ruff "
    "pre-commit checks. Anyway it is advised to recheck your files for potential issues. "
    "If you are sure you feel comfortable committing the files once again, it should easily pass."
)


def main(argv: list[str]) -> int:
    files = [Path(arg) for arg in argv]
    before: dict[Path, str | None] = {path: file_hash(path) for path in files}

    cmd = [sys.executable, "-m", "ruff", "format", *[str(p) for p in files]]
    completed = subprocess.run(cmd)
    if completed.returncode != 0:
        return completed.returncode

    changed = []
    for path in files:
        after = file_hash(path)
        if before[path] != after:
            changed.append(path)

    if changed:
        print(MESSAGE)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
