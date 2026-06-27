# Runtime Diagnosis

This is the canonical watchlist for runtime risks that affect the current app. Older diagnosis notes may exist locally or under `docs/historical/`, but this file is the tracked repo surface.

## Current Watchlist

| Area                  | Risk                                                                      | Current stance                                                              |
| --------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| AppKit pointers       | Window/view reuse can crash if stale pointers are trusted.                | Validate stored window pointers and clear state on close.                   |
| Hotkeys               | Modifier-only detection depends on CGEventTap and macOS permissions.      | Keep Accessibility and Input Monitoring prompts honest.                     |
| VAD auto-send         | Toggle mode can feel too aggressive if silence timing is wrong.           | Treat as tuning unless mode routing regresses.                              |
| Agent provider chains | `previous_response_id` can become poisoned after failed turns.            | Reset failed chains and replay safely when needed.                          |
| Tool activity         | Tool logs can overwhelm the conversation if rendered as primary messages. | Keep grouped Tool Activity compact and raw payloads debug-only.             |
| Release artifacts     | Local green checks do not prove DMG usability.                            | Mount, launch, and test signed/notarized artifacts outside dev environment. |

## Fast Triage

```bash
make status
make logs
make test-quick
```

For release-impacting changes:

```bash
make check
make install-app
```

Then launch the installed app and verify onboarding, permissions, hotkeys, dictation overlay, settings, and assistive overlay.

## Known Historical Fixes

- Stale overlay window pointer reuse was mitigated by validating stored pointers and clearing close state.
- Duplicate quality daemon spawns were mitigated with PID-file reuse and stale PID cleanup.
- VAD auto-finish spam was narrowed to toggle-mode behavior instead of hold-mode interruption.
