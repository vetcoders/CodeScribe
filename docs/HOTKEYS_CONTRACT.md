# Hotkeys Contract

CodeScribe uses a low-level macOS CGEventTap to detect modifier-only hotkeys. Bindings are mode-first and persisted in `~/Library/Application Support/CodeScribe/settings.json`.

## Mode Bindings

| Work mode  | Default binding     | Behavior                                                         |
| ---------- | ------------------- | ---------------------------------------------------------------- |
| Dictation  | `HoldFn`            | Hold to record, release to finalize and paste.                   |
| Formatting | `DoubleLeftOption`  | Toggle hands-free capture, run formatting, paste.                |
| Assistive  | `DoubleRightOption` | Toggle hands-free capture into the assistive overlay/agent lane. |

Users can change bindings in Settings > Modes & Shortcuts. Legacy `.env` keys such as `HOLD_MODS` and `TOGGLE_TRIGGER` are not the binding contract.

## Hold Mode

Hold mode starts recording on key down and finalizes on key up.

Default Fn modifiers:

| Gesture      | Meaning                    |
| ------------ | -------------------------- |
| Fn           | Dictation/raw capture.     |
| Fn + Shift   | Assistive/chat capture.    |
| Fn + Command | Selection/context capture. |

If `HOLD_EXCLUSIVE=1`, extra modifiers are ignored and Fn behaves as raw hold dictation only.

## Toggle Mode

Toggle mode starts on a double tap and sends/stops on the next double tap. It can also auto-send accumulated utterances after `TOGGLE_SILENCE_SEC` of silence.

| Binding             | Default use                     |
| ------------------- | ------------------------------- |
| `DoubleLeftOption`  | Formatting.                     |
| `DoubleRightOption` | Assistive.                      |
| `DoubleCtrl`        | Optional raw dictation binding. |
| `Disabled`          | No trigger for that work mode.  |

## VAD Contract

| Mode      | VAD role                                                                                                      |
| --------- | ------------------------------------------------------------------------------------------------------------- |
| Hold      | User controls capture boundary; VAD may segment speech internally, but it must not force-finish hold capture. |
| Toggle    | Silence boundaries can auto-send accumulated utterances without stopping the mode.                            |
| Assistive | Uses the same capture substrate, then routes output into the voice/agent overlay.                             |

## Runtime Knobs

| Variable                 | Default | Reload  |
| ------------------------ | ------- | ------- |
| `HOLD_START_DELAY_MS`    | `800`   | Restart |
| `DOUBLE_TAP_INTERVAL_MS` | `200`   | Restart |
| `TOGGLE_SILENCE_SEC`     | `5.0`   | Restart |
| `HOLD_EXCLUSIVE`         | `0`     | Restart |

VAD internals are configured in code, not through runtime env knobs.

## Code Surfaces

| File                      | Responsibility                                             |
| ------------------------- | ---------------------------------------------------------- |
| `app/os/hotkeys/`         | Detector, runtime config, platform CGEventTap integration. |
| `app/controller/`         | State machine and routing for hotkey events.               |
| `core/config/settings.rs` | Persisted mode bindings.                                   |
| `app/ui/settings/`        | User-facing binding controls.                              |
| `app/ui/onboarding/`      | First-run binding presets.                                 |

## Verification

```bash
cargo test -p codescribe --lib hotkey
cargo test --test e2e_vad_auto_stop
make test-quick
```
