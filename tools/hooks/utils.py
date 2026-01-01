"""Shared utilities for pre-commit hooks."""

from __future__ import annotations

import hashlib
from pathlib import Path


def file_hash(path: Path) -> str | None:
    """Return SHA-256 hash of file contents, or None if file not found."""
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except FileNotFoundError:
        return None
