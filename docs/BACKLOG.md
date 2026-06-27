# CodeScribe Backlog

This is the current active backlog. Historical roadmaps and future visions live under `docs/historical/`.

## Current Focus

| Priority | Item                                                         | Why it matters                                                                                         |
| -------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| P0       | Keep docs and public surfaces aligned with current code.     | Stale docs have been the main drift source for humans and agents.                                      |
| P0       | Finish public release gate for `0.12.x`.                     | The source version is ahead of the latest known public artifact.                                       |
| P0       | Verify signed/notarized DMGs outside the developer machine.  | Local builds do not prove Gatekeeper or onboarding success.                                            |
| P1       | Exercise Assistive Tool Activity in a live app conversation. | Unit coverage exists; final UX truth requires seeing a real turn with successful and failed tools.     |
| P1       | Keep Agentic readiness honest.                               | Missing Vibecrafted/AICX/Loctree/PRView substrate must block Agentic readiness, not silently degrade.  |
| P1       | Tighten AppKit runtime guards.                               | UI surfaces are pointer-heavy and need regression coverage around window reuse and close/reopen paths. |
| P2       | Revisit VAD timing defaults after real use.                  | Toggle mode depends on silence timing feeling right, not just tests passing.                           |

## Completed In Current Branch Family

| Area                      | Current truth                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------------ |
| OpenAI Responses defaults | Formatting uses `gpt-4.1`; Assistive uses `gpt-5.5`; endpoint defaults to `/v1/responses`. |
| Onboarding lanes          | Basic is the safe default; Agentic is explicit and persisted.                              |
| Agentic readiness         | Vibecrafted, AICX, Loctree, and PRView are classified as required substrate.               |
| Assistive timeline        | Tool calls are grouped into one Tool Activity block per assistant turn.                    |
| Attachments               | Image blocks are guarded so empty images are not sent as provider input.                   |
| Runtime docs              | Canonical docs are reduced to a small current set; older docs are quarantined.             |

## Historical Or Deferred

These are preserved as context under `docs/historical/`, not active implementation commitments:

- Apple Speech as primary live layer.
- `tail_patcher` and `final_bam` layered pipeline proposals.
- Speech-to-speech / Moshi product vision.
- Tauri Voice Lab.
- Libraxis Qube Protocol plans.
- Old guide/ADR duplicates.

## Verification Discipline

For code changes:

```bash
make test-quick
make check
```

For release or product-surface changes, also run the real app path and verify onboarding, permissions, hotkeys, dictation overlay, settings, and assistive overlay.
