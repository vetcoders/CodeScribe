# CodeScribe Configuration

This is the canonical configuration reference for current CodeScribe. Older env inventories live in `docs/historical/`.

## Configuration Sources

| Source                                                   | Purpose                                              |
| -------------------------------------------------------- | ---------------------------------------------------- |
| Process environment                                      | Highest-priority overrides for current process.      |
| `~/.codescribe/.env`                                     | Power-user overrides. Editing requires app restart.  |
| `~/Library/Application Support/CodeScribe/settings.json` | GUI-managed user settings.                           |
| macOS Keychain                                           | API keys under service `com.vetcoders.codescribe`.   |
| Built-in defaults                                        | Safe fallback values from `core/config/defaults.rs`. |

## Provider Defaults

| Setting            | Default                               |
| ------------------ | ------------------------------------- |
| Responses endpoint | `https://api.openai.com/v1/responses` |
| Formatting model   | `gpt-4.1`                             |
| Assistive model    | `gpt-5.5`                             |

Formatting and assistive mode can share `LLM_*` values or use mode-specific overrides.

```env
LLM_ENDPOINT=https://api.openai.com/v1/responses
LLM_MODEL=gpt-4.1
LLM_API_KEY=sk-proj-...

LLM_FORMATTING_ENDPOINT=https://api.openai.com/v1/responses
LLM_FORMATTING_MODEL=gpt-4.1
LLM_FORMATTING_API_KEY=sk-proj-...

LLM_ASSISTIVE_ENDPOINT=https://api.openai.com/v1/responses
LLM_ASSISTIVE_MODEL=gpt-5.5
LLM_ASSISTIVE_API_KEY=sk-proj-...
```

## Hotkeys

Mode bindings are not configured in `.env`. They live in `settings.json` and are managed by Settings > Modes & Shortcuts.

Remaining hotkey timing knobs:

| Variable                 | Default | Reload  |
| ------------------------ | ------- | ------- |
| `HOLD_START_DELAY_MS`    | `800`   | Restart |
| `DOUBLE_TAP_INTERVAL_MS` | `200`   | Restart |
| `TOGGLE_SILENCE_SEC`     | `5.0`   | Restart |
| `HOLD_EXCLUSIVE`         | `0`     | Restart |

## Local STT

```env
WHISPER_LANGUAGE=en
CODESCRIBE_MODEL_PATH=/path/to/whisper-large-v3-turbo-mlx-q8
USE_LOCAL_STT=1
```

`CODESCRIBE_MODEL_PATH` is only needed when the runtime lookup cannot find Whisper through the normal cache/model paths.

## Streaming And Overlay

| Variable                               | Meaning                                         |
| -------------------------------------- | ----------------------------------------------- |
| `CODESCRIBE_STREAM_CHUNK_SEC`          | Chunk length for live processing.               |
| `CODESCRIBE_STREAM_OVERLAP_RATIO`      | Chunk overlap ratio.                            |
| `CODESCRIBE_MAX_INFERENCE_CONCURRENCY` | Whisper inference concurrency, clamped by code. |
| `CODESCRIBE_BUFFER_DELAY_MS`           | Presentation buffering delay.                   |
| `CODESCRIBE_TYPING_CPS`                | Buffered typing speed.                          |
| `CODESCRIBE_OVERLAY_STABLE_PREVIEW`    | Show only stable preview when enabled.          |

## Storage

| Variable              | Meaning                    |
| --------------------- | -------------------------- |
| `CODESCRIBE_DATA_DIR` | Override `~/.codescribe`.  |
| `CODESCRIBE_ENV_PATH` | Override `.env` location.  |
| `HISTORY_ENABLED`     | Transcript history toggle. |
| `DUMP_AUDIO_LOGS`     | Persist paired audio logs. |

## Agentic Mode

Basic mode is the safe default. Agentic mode is only ready when these substrates are configured:

- Vibecrafted runtime.
- AICX MCP.
- Loctree MCP.
- PRView integration.

The readiness probe reads `~/.codescribe/mcp.json` and runtime discovery cache. Missing config is neutral for Basic mode but blocks Agentic readiness.

## Verification

```bash
make test-quick
make check
```
