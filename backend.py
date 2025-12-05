"""Compatibility alias so `import backend` maps to `codescribe.backend`."""

import sys

from codescribe import backend as _backend

sys.modules[__name__] = _backend
