from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any
import json


class ConnectionState(str, Enum):
    DISCONNECTED = "disconnected"
    CONNECTING = "connecting"
    CONNECTED = "connected"
    RECONNECTING = "reconnecting"
    ERROR = "error"


class PanelEventKind(str, Enum):
    MESSAGE = "message"
    STATUS = "status"
    PROGRESS = "progress"
    ERROR = "error"
    SESSION = "session"
    COMPLETIONS = "completions"
    UI_HINT = "ui_hint"
    TOOL = "tool"
    RAW = "raw"


@dataclass
class CompletionItem:
    value: str
    label: str = ""
    detail: str = ""
    kind: str = "command"

    @classmethod
    def from_any(cls, value: Any) -> "CompletionItem":
        if isinstance(value, str):
            return cls(value=value, label=value)
        if isinstance(value, dict):
            val = str(value.get("value") or value.get("insertText") or value.get("label") or "")
            return cls(
                value=val,
                label=str(value.get("label") or val),
                detail=str(value.get("detail") or value.get("description") or ""),
                kind=str(value.get("kind") or "command"),
            )
        return cls(value=str(value), label=str(value))


@dataclass
class PanelEvent:
    kind: PanelEventKind
    text: str = ""
    role: str = "jcode"
    session_id: str = ""
    progress: float | None = None
    completions: list[CompletionItem] = field(default_factory=list)
    ui: dict[str, Any] = field(default_factory=dict)
    raw: dict[str, Any] | None = None

    @property
    def is_error(self) -> bool:
        return self.kind == PanelEventKind.ERROR


def parse_panel_event(line: str) -> PanelEvent:
    """Parse the structured jcode-to-panel event contract with plain-text fallback."""
    try:
        data = json.loads(line)
    except Exception:
        return PanelEvent(kind=PanelEventKind.RAW, text=line, raw=None)

    if not isinstance(data, dict):
        return PanelEvent(kind=PanelEventKind.RAW, text=str(data), raw={"value": data})

    typ = str(data.get("type") or data.get("kind") or "message")
    if typ.startswith("panel."):
        typ = typ.removeprefix("panel.")

    aliases = {
        "assistant": PanelEventKind.MESSAGE,
        "assistant_message": PanelEventKind.MESSAGE,
        "response": PanelEventKind.MESSAGE,
        "delta": PanelEventKind.MESSAGE,
        "message": PanelEventKind.MESSAGE,
        "status": PanelEventKind.STATUS,
        "progress": PanelEventKind.PROGRESS,
        "error": PanelEventKind.ERROR,
        "session": PanelEventKind.SESSION,
        "completions": PanelEventKind.COMPLETIONS,
        "completion": PanelEventKind.COMPLETIONS,
        "ui_hint": PanelEventKind.UI_HINT,
        "tool": PanelEventKind.TOOL,
    }
    kind = aliases.get(typ, PanelEventKind.RAW)
    completions = [CompletionItem.from_any(x) for x in data.get("items", [])] if kind == PanelEventKind.COMPLETIONS else []
    return PanelEvent(
        kind=kind,
        text=_extract_text(data),
        role=str(data.get("role") or data.get("speaker") or "jcode"),
        session_id=str(data.get("session_id") or data.get("sessionId") or data.get("session") or ""),
        progress=_coerce_progress(data.get("progress") or data.get("percent")),
        completions=completions,
        ui=data.get("ui", {}) if isinstance(data.get("ui", {}), dict) else {},
        raw=data,
    )


def _coerce_progress(value: Any) -> float | None:
    if value is None or value == "":
        return None
    try:
        number = float(value)
    except Exception:
        return None
    if number > 1:
        number = number / 100.0
    return max(0.0, min(1.0, number))


def _extract_text(data: dict[str, Any]) -> str:
    for key in ("text", "content", "message", "delta", "output"):
        value = data.get(key)
        if isinstance(value, str) and value:
            return value
    # Some event streams use nested message/content arrays. Keep this generic
    # and conservative so unknown JSON does not render as empty "raw".
    message = data.get("message")
    if isinstance(message, dict):
        return _extract_text(message)
    content = data.get("content")
    if isinstance(content, list):
        parts: list[str] = []
        for item in content:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict):
                text = _extract_text(item)
                if text:
                    parts.append(text)
        return "".join(parts)
    return ""


def event_preview(event: PanelEvent, debug: bool = False) -> str:
    if debug and event.raw:
        return json.dumps(event.raw, ensure_ascii=False)[:160]
    if event.kind == PanelEventKind.ERROR:
        return f"Error: {event.text}"[:160]
    if event.kind == PanelEventKind.PROGRESS:
        pct = f" {int(event.progress * 100)}%" if event.progress is not None else ""
        return f"{event.text}{pct}"[:160]
    if event.kind == PanelEventKind.STATUS:
        return event.text[:160]
    if event.kind == PanelEventKind.SESSION and event.session_id:
        return f"Session: {event.session_id}"
    return (event.text or event.kind.value)[:160]
