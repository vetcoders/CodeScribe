# CodeScribe Truth Contract

CodeScribe must preserve user intent without silently mixing preview, transcript, formatting, and assistant interpretation.

## Vocabulary

| Term                     | Meaning                                                                       |
| ------------------------ | ----------------------------------------------------------------------------- |
| Live preview             | Local provisional text shown during recording.                                |
| Committed utterance      | Text segment already accepted by the stream pipeline.                         |
| Final transcript         | The chosen transcript after capture finishes.                                 |
| Formatted transcript     | Final transcript after optional AI cleanup.                                   |
| Assistant interpretation | Agent response based on transcript, selection, attachments, or tools.         |
| Tool activity            | Evidence of tool execution within an assistant turn.                          |
| Fallback                 | Alternate path used because the primary path was unavailable or insufficient. |
| Low confidence           | Transcript exists but the system has hard quality warnings.                   |

## Rules

1. Live preview is not final truth.
2. Final transcript provenance must remain visible to runtime/state.
3. Formatting may improve text, but it must not erase raw transcript truth.
4. Assistant output is not a transcript.
5. Tool calls are evidence, not primary conversation messages.
6. Raw tool payloads belong in logs/debug surfaces, not normal chat.
7. Fallbacks must be named as fallbacks.
8. Weak or missing speech must not auto-paste as confident output.
9. Agentic readiness must not fake green when required substrate is absent.
10. Historical docs must not present proposals as current implementation.

## Runtime Surfaces

- Transcript and confidence contracts: `core/pipeline/contracts.rs`.
- Controller routing and paste/assistive decisions: `app/controller/`.
- Formatting and Responses API state: `core/llm/`, `app/agent/openai_provider.rs`.
- Assistive tool activity: `app/ui/voice_chat/tool_activity.rs`.
- Agentic readiness: `app/agent/tools/mcp.rs`.

## Human Check

Before shipping a change, answer:

- Can the user tell preview from final output?
- Can the user tell transcript from assistant response?
- Are fallbacks named?
- Is a failed tool shown as failed?
- Does the doc say what the code actually does today?
