from __future__ import annotations

from dataclasses import dataclass, field
from .protocol import PanelEvent, PanelEventKind, event_preview


@dataclass
class ConversationBuffer:
    max_messages: int = 20
    messages: list[tuple[str, str]] = field(default_factory=list)
    _streaming_index: int | None = None

    def add_user(self, text: str) -> None:
        self._streaming_index = None
        self._append("You", text)

    def add_event(self, event: PanelEvent) -> None:
        text = event.text.strip("\n")
        if event.kind == PanelEventKind.SESSION:
            return
        if event.kind == PanelEventKind.STATUS:
            preview = event_preview(event).strip("\n")
            # Keep noisy protocol/status events out of chat unless useful.
            if preview and preview not in {"message_end"} and not preview.startswith("websocket/"):
                self._append_or_replace_status(preview)
            if event.raw and event.raw.get("type") == "message_end":
                self._streaming_index = None
            return
        if event.kind == PanelEventKind.RAW and not text:
            return
        if event.kind == PanelEventKind.ERROR:
            self._streaming_index = None
            self._append("jcode", "Error: " + text)
            return
        if event.kind == PanelEventKind.MESSAGE:
            if not text:
                return
            # `done` repeats the full accumulated text after deltas. Ignore it
            # when it equals the current streaming message to avoid duplication.
            if event.raw and event.raw.get("type") == "done" and self.messages:
                who, current = self.messages[-1]
                if who == "jcode" and (text == current or current.endswith(text)):
                    self._streaming_index = None
                    return
            if event.raw and event.raw.get("type") == "done" and self._streaming_index is not None:
                who, current = self.messages[self._streaming_index]
                if text == current or current.endswith(text):
                    self._streaming_index = None
                    return
                self.messages[self._streaming_index] = (who, text)
                self._streaming_index = None
                return
            if self._streaming_index is not None and self._streaming_index < len(self.messages):
                who, current = self.messages[self._streaming_index]
                self.messages[self._streaming_index] = (who, current + text)
            else:
                self._append("jcode", text)
                self._streaming_index = len(self.messages) - 1
            return
        preview = event_preview(event)
        if preview and preview != "raw":
            self._append("jcode", preview)

    def _append_or_replace_status(self, text: str) -> None:
        # Avoid filling chat with connection-phase noise. Show only latest status
        # if there is no active assistant stream.
        if self._streaming_index is None and (not self.messages or self.messages[-1][0] == "status"):
            if self.messages and self.messages[-1][0] == "status":
                self.messages[-1] = ("status", text)
            else:
                self._append("status", text)

    def _append(self, who: str, text: str) -> None:
        self.messages.append((who, text))
        if len(self.messages) > self.max_messages:
            overflow = len(self.messages) - self.max_messages
            self.messages = self.messages[-self.max_messages :]
            if self._streaming_index is not None:
                self._streaming_index = max(0, self._streaming_index - overflow)

    def latest_preview(self, debug: bool = False) -> str:
        if not self.messages:
            return "jcode-panel ready"
        who, text = self.messages[-1]
        if debug:
            return f"{who}: {text}"[:120]
        lowered = text.lower()
        if "error" in lowered or "failed" in lowered:
            return "Error: " + text[:100]
        if who == "status" or any(x in lowered for x in ["running", "building", "compiling", "testing", "sending"]):
            return text[:120]
        return f"{who}: {text}"[:120]
