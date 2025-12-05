# ruff: noqa: E402
"""CodeScribe tray entry point.

This module intentionally stays tiny and hands off all heavy lifting to
`codescribe.app.runtime` so the historical import path keeps working while the
runtime lives in a dedicated module.
"""

from __future__ import annotations

from dotenv import load_dotenv

from .path_utils import repo_root

# Ensure local .env is loaded before any runtime modules read os.environ
_env_path = repo_root() / ".env"
if _env_path.exists():
    load_dotenv(dotenv_path=_env_path)
else:
    load_dotenv()

from .app.runtime import CodeScribe, acquire_lock, run

__all__ = ["CodeScribe", "acquire_lock", "run", "main"]


def main() -> None:
    """Boot the CodeScribe tray application."""
    run()


if __name__ == "__main__":
    main()
