from __future__ import annotations

from dataclasses import dataclass, field
from .protocol import PanelEvent, PanelEventKind, event_preview


@dataclass
class ConversationBuffer:
    max_messages: int = 20
    messages: list[tuple[str, str]] = field(default_factory=list)

    def add_user(self, text: str) -> None:
        self._append("You", text)

    def add_event(self, event: PanelEvent) -> None:
        if event.kind == PanelEventKind.SESSION and not event.text:
            return
        who = event.role if event.role else "jcode"
        self._append(who, event.text or event_preview(event))

    def _append(self, who: str, text: str) -> None:
        self.messages.append((who, text))
        if len(self.messages) > self.max_messages:
            self.messages = self.messages[-self.max_messages :]

    def latest_preview(self, debug: bool = False) -> str:
        if not self.messages:
            return "jcode-panel ready"
        who, text = self.messages[-1]
        if debug:
            return f"{who}: {text}"[:120]
        lowered = text.lower()
        if "error" in lowered or "failed" in lowered:
            return "Error: " + text[:100]
        if any(x in lowered for x in ["running", "building", "compiling", "testing"]):
            return text[:120]
        return f"{who}: {text}"[:120]
