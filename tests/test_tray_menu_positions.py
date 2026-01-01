import importlib
import sys

import pytest

from tests.helpers.fake_hotkeys import build_fake_hotkeys_module
from tests.helpers.fake_rumps import MenuItem, build_fake_rumps_module

MODULES_WITH_RUMPS = [
    "codescribe.permission_manager",
    "codescribe.menu_formatting",
    "codescribe.app.status",
    "codescribe.app.controllers.history",
    "codescribe.app.controllers.models",
    "codescribe.app.menu_utils",
    "codescribe.app.mixins.appearance",
    "codescribe.app.mixins.backends",
    "codescribe.app.mixins.feedback",
    "codescribe.app.mixins.hold_menu",
    "codescribe.app.mixins.runtime_loop",
    "codescribe.app.mixins.tools",
    "codescribe.app.recording_controller",
]


class DummyRecorder:
    def __init__(self):
        self.last_duration = 0.0

    async def stop(self):  # pragma: no cover - tests never await
        return None


@pytest.fixture
def tray_runtime(monkeypatch):
    fake_rumps = build_fake_rumps_module()
    fake_hotkeys = build_fake_hotkeys_module()
    monkeypatch.setitem(sys.modules, "rumps", fake_rumps)
    monkeypatch.setitem(sys.modules, "codescribe.hotkeys", fake_hotkeys)

    import codescribe.audio as audio

    monkeypatch.setattr(audio, "Recorder", DummyRecorder)

    import codescribe.first_run as first_run

    monkeypatch.setattr(first_run, "ensure_config_and_permissions", lambda: None)

    import codescribe.diag as diag

    monkeypatch.setattr(diag, "run_preflight", lambda _logger: {})
    monkeypatch.setattr(diag, "write_snapshot", lambda _info, _root: None)

    saved = {}
    for name in MODULES_WITH_RUMPS:
        saved[name] = sys.modules.pop(name, None)
    sys.modules.pop("codescribe.app.runtime", None)

    runtime = importlib.import_module("codescribe.app.runtime")

    yield runtime, fake_rumps

    sys.modules.pop("codescribe.app.runtime", None)
    for name in MODULES_WITH_RUMPS:
        sys.modules.pop(name, None)
        if saved[name] is not None:
            sys.modules[name] = saved[name]
    sys.modules.pop("rumps", None)
    sys.modules.pop("codescribe.hotkeys", None)


def _visible_titles(menu, fake_rumps_module):
    titles = []
    for entry in menu.ordered():
        if isinstance(entry, fake_rumps_module.MenuItem):
            titles.append(entry.title)
    return titles


def test_tray_menu_order_and_callbacks(tray_runtime):
    runtime, fake_rumps = tray_runtime
    app = runtime.CodeScribe()

    titles = _visible_titles(app.menu, fake_rumps)
    expected = [
        "Status: Initializing...",
        "Enable Hotkeys",
        "Language",
        "Formatting",
        "Hold Hotkeys",
        "Models",
        "Backends",
        "History",
        "Appearance",
        "Feedback",
        "Permissions",
        "Tools",
        "What do these toggles do?",
        "Start at Login",
        "Quit...",
    ]
    assert titles == expected

    for label in ("Enable Hotkeys", "What do these toggles do?", "Start at Login", "Quit..."):
        item = app.menu[label]
        assert isinstance(item, MenuItem)
        assert callable(item.callback)

    for submenu in (
        app.menu_models,
        app.menu_backends,
        app.menu_history,
        app.menu_permissions,
        app.menu_tools,
    ):
        assert submenu.menu is not None
        assert any(
            isinstance(child, MenuItem) for child in submenu.menu.ordered() if child is not None
        )
