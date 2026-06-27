# Assistive Agent

Assistive mode is CodeScribe's dictation-driven orchestration lane. It is not just "chat next to dictation": it captures speech, selection, attachments, conversation state, and optional tools into one assistant turn.

## Modes

| Lane    | Meaning                                                                                                               |
| ------- | --------------------------------------------------------------------------------------------------------------------- |
| Basic   | Safe default. Dictation and formatting work without requiring the agentic substrate.                                  |
| Agentic | Opt-in lane for Vibecrafted/AICX/Loctree/PRView-backed operation. Readiness is checked before it is treated as ready. |

The persisted first-run choice lives in `settings.json` as `basic` or `agentic`. Unknown or corrupt values fall back to Basic.

## Provider

Assistive mode uses OpenAI Responses API by default:

```env
LLM_ASSISTIVE_ENDPOINT=https://api.openai.com/v1/responses
LLM_ASSISTIVE_MODEL=gpt-5.5
LLM_ASSISTIVE_API_KEY=sk-proj-...
```

The provider uses `previous_response_id` for conversation chaining when available, and resets poisoned chains instead of carrying failed state forward.

## Tool Activity Contract

The primary conversation timeline is for conversation. Tool calls are grouped as evidence:

- one compact Tool Activity block per assistant turn,
- friendly labels from `friendly_tool_name`,
- running/completed/failed state per tool,
- raw MCP wire names and full payloads kept debug-only.

Current implementation:

- `app/controller/helpers.rs` maps raw tool names and records tool events.
- `app/ui/voice_chat/tool_activity.rs` groups and renders per-turn activity.
- `app/ui/voice_chat/state.rs` owns tool activity state for the active turn.

## Agentic Readiness

Agentic mode requires:

- Vibecrafted runtime,
- AICX MCP,
- Loctree MCP,
- PRView integration.

The readiness probe reads `~/.codescribe/mcp.json` and runtime discovery. Missing config is neutral for Basic mode but blocks Agentic readiness.

## Attachments

Assistive turns can include attachments such as images. Empty image blocks are not sent, and image limits are enforced before provider submission.

## Verification

Relevant targeted checks:

```bash
cargo test -p codescribe --lib tool_activity
cargo test -p codescribe --lib voice_chat::api::tests
cargo test -p codescribe --lib onboarding
```
