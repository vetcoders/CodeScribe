# CodeScribe Tauri Migration Plan

## Overview

Przepisanie frontendu CodeScribe z Python (rumps/Quartz/pyobjc) na Rust (Tauri).
Backend Python (FastAPI + MLX Whisper) **zostaje bez zmian**.

## Architektura docelowa

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri App (Rust)                         │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Tray UI    │  │   Hotkeys    │  │  Audio Recorder  │  │
│  │   (native)   │  │ (rdev/tao)   │  │  (cpal/rodio)    │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│                           │                                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              State Machine / Controller               │  │
│  │     IDLE → REC_HOLD/REC_TOGGLE → BUSY → IDLE         │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                   HTTP Client                         │  │
│  │     POST /transcribe, POST /format, GET /healthz      │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                Clipboard (arboard)                    │  │
│  │          paste_text() → Cmd+V simulation             │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ HTTP
┌─────────────────────────────────────────────────────────────┐
│              Python Backend (existing)                       │
│                                                             │
│  FastAPI: /transcribe, /format, /healthz, /ws/transcribe   │
│  MLX Whisper (STT) + Light+ + Harmony/Ollama (formatting)  │
└─────────────────────────────────────────────────────────────┘
```

## Moduły do zaimplementowania

### 1. `src-tauri/src/hotkeys.rs`
**Odpowiednik**: `src/codescribe/hotkeys.py` (529 linii)

Funkcjonalność:
- Hold Ctrl (+ opcjonalne modyfikatory: Alt, Shift, Cmd)
- Double-tap Option (z konfigurowalnym interwałem)
- Exclusive mode (tylko wymagane modyfikatory, bez dodatkowych)
- Event queue: `("hold", "down/up", assistive)`, `("toggle", "press")`

Biblioteki Rust:
- **rdev** - cross-platform keyboard/mouse events, low-level
- Alternatywnie: **tao** events (wbudowane w Tauri)

State:
```rust
struct HotkeyState {
    last_combo_down: bool,
    last_alt_down_ts: Option<Instant>,
    required_hold_mask: ModifierFlags,
    exclusive_mode: bool,
    non_modifier_keys_down: HashSet<u16>,
}
```

### 2. `src-tauri/src/audio.rs`
**Odpowiednik**: `src/codescribe/audio.py` (394 linie)

Funkcjonalność:
- Nagrywanie 16kHz mono int16
- Silence detection (RMS threshold)
- Auto-stop po ciszy
- `snapshot_wav()` dla live streaming
- Zapis do temp WAV

Biblioteki Rust:
- **cpal** - cross-platform audio I/O
- **hound** - WAV encoding/decoding

```rust
struct Recorder {
    stream: Option<cpal::Stream>,
    buffer: Vec<i16>,
    config: RecorderConfig,
}

impl Recorder {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<PathBuf>;
    fn snapshot_wav(&self, min_seconds: f32) -> Option<PathBuf>;
}
```

### 3. `src-tauri/src/client.rs`
**Odpowiednik**: `src/codescribe/client.py` (528 linii)

Funkcjonalność:
- Server discovery (probe ports: 8237, 7237, 6237, 5237)
- `transcribe_http(audio_path, language)` → POST /transcribe
- `format_text_http(text, assistive)` → POST /format
- Health check
- Retry logic z exponential backoff

Biblioteki Rust:
- **reqwest** - HTTP client (async)

```rust
pub async fn transcribe(path: &Path, language: Option<&str>) -> Result<String>;
pub async fn format_text(text: &str, assistive: bool) -> Result<String>;
pub async fn resolve_server_url() -> Option<String>;
pub fn check_server_status() -> ServerStatus;
```

### 4. `src-tauri/src/controller.rs`
**Odpowiednik**: `src/codescribe/app/recording_controller.py` (570 linii)

State machine:
```
IDLE ─── hold_down ──→ (delay 800ms) ──→ REC_HOLD
  │                                          │
  │                                     hold_up
  │                                          │
  ├─── toggle_press ──→ REC_TOGGLE          │
  │                          │              │
  │                     toggle_press        │
  │                          │              │
  └──────────────────────────┴──────→ BUSY ──→ IDLE
                                        │
                              transcribe + format + paste
```

Funkcjonalność:
- Delayed start (800ms dla hold)
- Live streaming podczas REC_HOLD
- Paste do clipboard + Cmd+V simulation
- Fallback recording archival

### 5. `src-tauri/src/tray.rs`
**Odpowiednik**: `src/codescribe/app/runtime.py` + mixins

Funkcjonalność:
- System tray icon z menu
- Status glyph: • (idle), ◉ (listen), … (think), ✓ (success)
- Menu structure (uproszczone na start):
  - Status
  - Language (Auto/PL/EN)
  - Formatting toggle
  - Hold mode config
  - History
  - Quit

Tauri:
- `tauri::SystemTray`
- `tauri::SystemTrayMenu`
- Custom icons lub unicode glyphs

### 6. `src-tauri/src/clipboard.rs`
**Odpowiednik**: fragmenty `src/codescribe/ui.py`

Funkcjonalność:
- Copy text to clipboard
- Simulate Cmd+V paste
- Restore previous clipboard (optional)

Biblioteki:
- **arboard** - cross-platform clipboard
- **enigo** lub **rdev** - key simulation

## Struktura projektu Tauri

```
codescribe-tauri/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── hotkeys.rs
│       ├── audio.rs
│       ├── client.rs
│       ├── controller.rs
│       ├── tray.rs
│       ├── clipboard.rs
│       ├── config.rs
│       └── lib.rs
├── src/                    # Frontend (opcjonalnie - settings UI)
│   ├── index.html
│   └── main.ts
├── package.json
└── README.md
```

## Fazy implementacji

### Faza 1: Skeleton + Tray (MVP)
- [ ] `cargo tauri init` - scaffold projektu
- [ ] Basic tray icon z menu Quit
- [ ] Health check do backendu Python
- [ ] Uruchomienie backendu przy starcie (sidecar lub spawn)

### Faza 2: Audio Recording
- [ ] cpal recorder z bufferem
- [ ] WAV encoding (hound)
- [ ] Silence detection
- [ ] Temp file handling

### Faza 3: HTTP Client
- [ ] Server discovery (probe ports)
- [ ] POST /transcribe
- [ ] POST /format
- [ ] Retry logic

### Faza 4: Hotkeys
- [ ] rdev/tao keyboard listener
- [ ] Hold Ctrl detection z delayed start
- [ ] Double-tap Option detection
- [ ] Exclusive mode

### Faza 5: Controller + Clipboard
- [ ] State machine
- [ ] Pipeline: record → transcribe → format → paste
- [ ] Clipboard manipulation + Cmd+V
- [ ] Tray icon updates

### Faza 6: Full Menu + Config
- [ ] Pełne menu z submenu
- [ ] Settings persistence (JSON)
- [ ] Language/model selection
- [ ] History (optional)

## Cargo.toml dependencies

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "shell-open"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json", "multipart"] }
cpal = "0.15"
hound = "3.5"
arboard = "3"
rdev = "0.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
directories = "5"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
```

## Migracja użytkowników

1. Backend Python pozostaje (`./CodeScribe start backend`)
2. Nowa aplikacja Tauri jako replacement dla tray
3. Konfiguracja kompatybilna (`~/.CodeScribe/settings.json`)
4. DMG z oboma komponentami

## Pytania do ustalenia

1. **Nazwa**: CodeScribe zostaje czy rebrand (np. "Scribe")?
2. **Frontend**: Czy potrzebujemy UI okno (settings) czy tylko tray?
3. **Cross-platform**: macOS-only na start czy od razu Windows/Linux?
4. **Packaging**: Jak bundlować backend Python z Tauri? (sidecar vs external)
