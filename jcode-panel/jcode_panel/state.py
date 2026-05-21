from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any
import os
import tempfile

from .config import STATE_PATH, _dump_simple_toml, _load_simple_toml

PROMPT_HISTORY_SEPARATOR = "|||JCODE_PANEL_PROMPT|||"


@dataclass
class AppState:
    """Mutable runtime state, separate from user preferences."""

    saved_session: str = ""
    saved_session_name: str = "jcode-panel"
    prompt_history: list[str] = field(default_factory=list)
    last_context_summary: str = ""
    last_token_stats: str = ""
    browser_bridge_seen: bool = False

    @classmethod
    def load(cls, path: Path = STATE_PATH) -> "AppState":
        if not path.exists():
            backup = path.with_name(path.name + ".bak")
            if not backup.exists():
                return cls()
            path = backup
        try:
            data = _load_simple_toml(path.read_text())
        except Exception:
            backup = path.with_name(path.name + ".bak")
            if not backup.exists() or backup == path:
                return cls()
            data = _load_simple_toml(backup.read_text())
        raw = data.get("state", {}) if isinstance(data, dict) else {}
        history = raw.get("prompt_history", [])
        if isinstance(history, str):
            history = [x for x in history.split(PROMPT_HISTORY_SEPARATOR) if x]
        return cls(
            saved_session=str(raw.get("saved_session", "")),
            saved_session_name=str(raw.get("saved_session_name", "jcode-panel")) or "jcode-panel",
            prompt_history=list(history) if isinstance(history, list) else [],
            last_context_summary=str(raw.get("last_context_summary", "")),
            last_token_stats=str(raw.get("last_token_stats", "")),
            browser_bridge_seen=bool(raw.get("browser_bridge_seen", False)),
        )

    def save(self, path: Path = STATE_PATH) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        data: dict[str, Any] = asdict(self)
        # Keep fallback TOML parser simple and deterministic.
        data["prompt_history"] = PROMPT_HISTORY_SEPARATOR.join(self.prompt_history[-100:])
        text = _dump_simple_toml({"state": data})
        fd, tmp_name = tempfile.mkstemp(prefix=path.name + ".", suffix=".tmp", dir=str(path.parent))
        try:
            with os.fdopen(fd, "w") as tmp:
                tmp.write(text)
                tmp.flush()
                os.fsync(tmp.fileno())
            os.replace(tmp_name, path)
            try:
                path.with_name(path.name + ".bak").write_text(text)
            except Exception:
                pass
        finally:
            try:
                if os.path.exists(tmp_name):
                    os.unlink(tmp_name)
            except Exception:
                pass

    def remember_prompt(self, prompt: str, limit: int = 100) -> None:
        prompt = prompt.strip()
        if not prompt:
            return
        if prompt in self.prompt_history:
            self.prompt_history.remove(prompt)
        self.prompt_history.append(prompt)
        self.prompt_history = self.prompt_history[-limit:]

    def set_saved_session(self, session: str) -> None:
        self.saved_session = session.strip()

    def set_saved_session_name(self, name: str) -> None:
        self.saved_session_name = name.strip() or "jcode-panel"

    def set_last_token_stats(self, stats: str) -> None:
        self.last_token_stats = stats.strip()
