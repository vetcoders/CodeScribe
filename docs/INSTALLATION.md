# CodeScribe Installation

This is the canonical install and launch guide for the current repository.

## Requirements

- macOS 14 or newer.
- Apple Silicon Mac.
- Rust toolchain with Rust 2024 support.
- `pipx` if you want local git hook setup through `pre-commit`.
- OpenAI API key for AI formatting or assistive mode.

Local-only dictation can run without an API key if local Whisper is available.

## Development Install

```bash
git clone https://github.com/VetCoders/CodeScribe.git
cd CodeScribe

pipx install pre-commit
make install
codescribe --version
```

`make install` installs the CLI and repo-local hooks. The standard build embeds Silero VAD and MiniLM support assets when available, then resolves Whisper from runtime lookup.

## App Install

```bash
make install-app
```

This builds `CodeScribe.app`, installs it to `/Applications`, and performs model/cache preparation as needed. It prefers a stable local signing identity when one exists and falls back to ad-hoc signing only when no usable identity is available.

## Release DMGs

```bash
make release-dmgs
```

Release DMGs must be Developer ID signed, notarized, stapled, and smoke-tested outside the developer environment before the landing page or README promises them as the primary path.

Expected variants:

- `CodeScribe_<version>.dmg`: standard build with support assets and runtime Whisper cache/download.
- `CodeScribe_<version>_full.dmg`: larger build with Whisper embedded too.

See `docs/RELEASE.md` for the public release gate.

## Runtime Configuration

CodeScribe uses tiered configuration:

```text
~/Library/Application Support/CodeScribe/settings.json
    GUI-managed settings.

macOS Keychain service com.vetcoders.codescribe
    API keys and secrets.

~/.codescribe/.env
    Optional power-user overrides.

~/.codescribe/prompts/
    Optional custom formatting and assistive prompts.

~/.codescribe/history/
    Transcript history.
```

Configuration priority is:

1. Process environment.
2. `~/.codescribe/.env`.
3. `settings.json`.
4. Built-in defaults.

## Launch

```bash
codescribe
make start
make stop
make logs
```

## Permissions

Grant these in System Settings > Privacy & Security:

| Permission       | Purpose                                        |
| ---------------- | ---------------------------------------------- |
| Microphone       | Audio recording.                               |
| Accessibility    | Hotkeys and paste automation.                  |
| Input Monitoring | Modifier-only hotkey detection.                |
| Screen Recording | Screenshot/vision-input agent tools when used. |

After changing macOS permissions, restart CodeScribe.

## Model Lookup

Runtime Whisper lookup order:

1. `CODESCRIBE_MODEL_PATH`.
2. `~/.codescribe/models/whisper-large-v3-turbo-mlx-q8/`.
3. `./models/whisper-large-v3-turbo-mlx-q8/`.
4. Hugging Face cache snapshots for `LibraxisAI/whisper-large-v3-turbo-mlx-q8`.

The required model files are `config.json`, `weights.safetensors`, `tokenizer.json`, and `mel_filters.npz`.

## Useful Commands

```bash
make help
make build
make release
make install
make install-app
make config
make check
make test-quick
```
