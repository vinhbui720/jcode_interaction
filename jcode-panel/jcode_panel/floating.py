from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class CompletionState:
    items: list[str] = field(default_factory=list)
    index: int = 0

    def update(self, items: list[str]) -> None:
        self.items = items
        self.index = 0

    def current(self) -> str:
        return self.items[self.index] if self.items else ""

    def tab(self) -> str:
        if not self.items:
            return ""
        value = self.items[self.index]
        self.index = (self.index + 1) % len(self.items)
        return value
