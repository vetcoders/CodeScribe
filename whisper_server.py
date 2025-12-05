"""Deprecated shim for `codescribe.whisper_server`.

Historically this module re-exported the FastAPI app by mutating ``sys.modules``.
We now simply import from the canonical package path and warn callers so they
can update their imports.
"""

from __future__ import annotations

import os
import warnings

from codescribe import whisper_server as _impl

warnings.warn(
    "Importing 'whisper_server' from the repository root is deprecated; "
    "use 'from codescribe import whisper_server' instead.",
    DeprecationWarning,
    stacklevel=2,
)

app = _impl.app
healthz = _impl.healthz
transcribe = _impl.transcribe
__all__ = ["app", "healthz", "transcribe", "main"]


def main() -> None:
    import uvicorn

    host = os.environ.get("HOST", "127.0.0.1")
    port = int(os.environ.get("PORT", "8238"))
    uvicorn.run("codescribe.whisper_server:app", host=host, port=port, reload=False)


if __name__ == "__main__":
    main()
