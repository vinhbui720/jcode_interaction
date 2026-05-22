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

    def save(self, path: Path = STATE_PATH, *, allow_clear_session: bool = False) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        if not allow_clear_session:
            self._preserve_existing_non_empty_values(path)
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

    def _preserve_existing_non_empty_values(self, path: Path) -> None:
        """Avoid clobbering durable runtime state with startup/default blanks.

        Startup and settings flows can construct a fresh AppState before jcode has
        reported a session or token usage. Those flows should not erase the last
        known resumable session/token badge. Explicit user actions, such as
        starting a new section, pass allow_clear_session=True to permit clearing.
        """
        try:
            existing = AppState.load(path)
        except Exception:
            return
        if not self.saved_session and existing.saved_session:
            self.saved_session = existing.saved_session
        if (not self.saved_session_name or self.saved_session_name == "jcode-panel") and existing.saved_session_name:
            self.saved_session_name = existing.saved_session_name
        if not self.last_token_stats and existing.last_token_stats:
            self.last_token_stats = existing.last_token_stats
        if not self.prompt_history and existing.prompt_history:
            self.prompt_history = existing.prompt_history
        if not self.last_context_summary and existing.last_context_summary:
            self.last_context_summary = existing.last_context_summary
        if not self.browser_bridge_seen and existing.browser_bridge_seen:
            self.browser_bridge_seen = existing.browser_bridge_seen
