import asyncio
import importlib

from codescribe.settings_store import reset_settings_for_tests


def test_stt_uses_remote_when_configured(monkeypatch, tmp_path):
    # Configure remote URL and stub HTTP post
    monkeypatch.setenv("WHISPER_SERVER_URL", "http://localhost:9999")

    import codescribe.stt as stt_mod

    importlib.reload(stt_mod)

    called = {}

    async def fake_http_post(url, files):
        called["url"] = url
        # emulate server response
        return {"text": "remote-transcript"}

    # patch helper used by stt when remote
    monkeypatch.setattr(
        stt_mod,
        "_http_post",
        lambda url, files: asyncio.get_event_loop().run_until_complete(fake_http_post(url, files)),
    )

    # create a dummy wav file path for code that reads it (not strictly required by remote)
    p = tmp_path / "a.wav"
    p.write_bytes(b"RIFF0000WAVEfmt ")

    out = asyncio.run(stt_mod.transcribe(str(p)))
    assert out["ok"] is True
    assert out["text"] == "remote-transcript"
    assert called["url"].endswith("/transcribe")


def test_llm_uses_remote_when_configured(monkeypatch, tmp_path):
    import codescribe.llm as llm_mod

    settings_path = tmp_path / "settings.json"
    settings_path.write_text(
        __import__("json").dumps({"ai_formatting_enabled": True, "ai_provider": "harmony"}),
        encoding="utf-8",
    )
    monkeypatch.setenv("CODESCRIBE_SETTINGS_PATH", str(settings_path))
    reset_settings_for_tests()

    importlib.reload(llm_mod)

    async def fake_harmony(text, assistive, settings):
        return text.upper()

    monkeypatch.setattr(llm_mod, "_format_with_harmony", fake_harmony)

    out = asyncio.run(llm_mod.format_text("ala ma kota"))
    assert out == "ALA MA KOTA."


def test_servers_apps_exist_and_healthz():
    import importlib

    from fastapi.testclient import TestClient

    whisper_server = importlib.import_module("codescribe.whisper_server")

    wc = TestClient(whisper_server.app)

    wr = wc.get("/healthz")
    assert wr.status_code == 200
    assert "ok" in wr.json()
