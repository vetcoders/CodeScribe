# ruff: noqa: E402
"""CodeScribe tray entry point.

This module intentionally stays tiny and hands off all heavy lifting to
`codescribe.app.runtime` so the historical import path keeps working while the
runtime lives in a dedicated module.
"""

from __future__ import annotations

from dotenv import load_dotenv

from .path_utils import repo_root, user_data_root

# Load .env files in order: repo defaults first, then user overrides
# 1. Load repo .env (development defaults)
_repo_env = repo_root() / ".env"
if _repo_env.exists():
    load_dotenv(dotenv_path=_repo_env)
else:
    load_dotenv()

# 2. Load user data .env (~/.CodeScribe/.env) - overrides repo settings
_user_env = user_data_root() / ".env"
if _user_env.exists():
    load_dotenv(dotenv_path=_user_env, override=True)

from .app.runtime import CodeScribe, acquire_lock, run

__all__ = ["CodeScribe", "acquire_lock", "run", "main"]


def main() -> None:
    """Boot the CodeScribe tray application."""
    run()


if __name__ == "__main__":
    main()
