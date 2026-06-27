# CodeScribe Documentation

This directory has one canonical documentation set plus a historical quarantine.

## Canonical Docs

| Doc                                              | Purpose                                                   |
| ------------------------------------------------ | --------------------------------------------------------- |
| [`../README.md`](../README.md)                   | Product overview and first install path.                  |
| [`ARCHITECTURE.md`](ARCHITECTURE.md)             | Current repo and runtime architecture.                    |
| [`INSTALLATION.md`](INSTALLATION.md)             | Install, app bundle, DMG, permissions, model lookup.      |
| [`CONFIGURATION.md`](CONFIGURATION.md)           | Settings, Keychain, `.env`, providers, Agentic readiness. |
| [`HOTKEYS_CONTRACT.md`](HOTKEYS_CONTRACT.md)     | Current hotkey modes and detector behavior.               |
| [`STREAMING_PIPELINE.md`](STREAMING_PIPELINE.md) | Current live transcription pipeline.                      |
| [`ASSISTIVE_AGENT.md`](ASSISTIVE_AGENT.md)       | Assistive mode, Tool Activity, Agentic readiness.         |
| [`RUNTIME_DIAGNOSIS.md`](RUNTIME_DIAGNOSIS.md)   | Current runtime watchlist.                                |
| [`RELEASE.md`](RELEASE.md)                       | Public release and artifact gate.                         |
| [`BACKLOG.md`](BACKLOG.md)                       | Active roadmap/backlog.                                   |
| [`TRUTH_CONTRACT.md`](TRUTH_CONTRACT.md)         | Product truth vocabulary and invariants.                  |

Together with this index, that is the canonical docs surface. Keep it small.

## Historical Docs

Everything under [`historical/`](historical/) is context, not current truth. Historical docs may contain useful reasoning, ADRs, audits, release drafts, and future concepts, but they must not be linked as implementation proof unless a canonical doc explicitly says so.

Examples of historical-only topics:

- Apple Speech primary layered pipeline.
- `tail_patcher` / `final_bam` proposals.
- old guide duplicates,
- old release notes and audit reports,
- future speech-to-speech agent designs.

## Rule For Future Edits

If a doc describes current behavior, keep it in the canonical set and verify it against code. If it describes intent, a proposal, an experiment, or stale history, put it under `docs/historical/`.
