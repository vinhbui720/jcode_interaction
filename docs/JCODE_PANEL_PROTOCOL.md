# jcode ↔ jcode-panel Protocol Contract

This document defines the panel-side contract that lets jcode core adapt and enhance the GUI without the panel reimplementing jcode logic.

Transport can be a formal local API or CLI subprocess streaming. The payload contract is JSON Lines where possible, with plain text fallback.

## Event shape

```json
{"type":"panel.message","role":"assistant","text":"Done"}
```

Common fields:

| Field | Meaning |
|---|---|
| `type` / `kind` | Event type. `panel.` prefix optional. |
| `text` / `content` / `message` | Human text to render. |
| `role` | `assistant`, `user`, `system`, `tool`, or `jcode`. |
| `session_id` | Current session id/name. Panel persists this when present. |
| `progress` / `percent` | 0..1 or 0..100 progress value. |
| `items` | Completion candidates for completion events. |
| `ui` | Optional GUI hints. |

## Event types

| Type | Purpose |
|---|---|
| `panel.message` | Chat message chunk/final message. |
| `panel.status` | Smart top-bar status such as `Running tests...`. |
| `panel.progress` | Progress update with optional percentage. |
| `panel.error` | User-visible error. |
| `panel.session` | Session id/name update. |
| `panel.completions` | Completion candidates for slash commands/skills/sessions. |
| `panel.ui_hint` | Safe GUI hints, e.g. ask panel to reveal context, open terminal, show settings. |
| `panel.tool` | Tool activity summary. |

## Completion event

```json
{
  "type": "panel.completions",
  "items": [
    {"value":"/grill-me", "label":"/grill-me", "detail":"Stress-test a plan", "kind":"skill"}
  ]
}
```

## UI hints

The core may suggest GUI affordances, but the panel decides whether they are safe.

Examples:

```json
{"type":"panel.ui_hint","ui":{"action":"open_terminal","session_id":"fox"}}
{"type":"panel.ui_hint","ui":{"action":"show_context_reveal"}}
{"type":"panel.ui_hint","ui":{"action":"show_settings","section":"hotkey"}}
```

Rules:

- No destructive action is executed automatically.
- Terminal opening is allowed only for local jcode commands.
- The panel never gives jcode arbitrary GUI automation power.

## Prompt input from panel to jcode

Preferred structured request:

```json
{
  "type":"panel.prompt",
  "text":"explain this page",
  "context": {
    "app":"Firefox",
    "window_title":"GitHub issue",
    "browser":{"url":"https://github.com/...", "title":"Issue", "selected_text":"..."}
  }
}
```

Fallback request is plain text with a compact `[Context]...[/Context]` prefix.
