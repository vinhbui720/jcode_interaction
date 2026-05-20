from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime

from .config import AppConfig
from .context import ActiveContext
from .state import AppState


@dataclass
class PromptRequest:
    text: str
    context: ActiveContext | None
    include_context: bool
    metadata_supported: bool = False


class PromptBuilder:
    """Builds outgoing user prompts without depending on GTK."""

    def build_text(self, request: PromptRequest) -> str:
        return request.text.strip()

    def build_metadata(self, request: PromptRequest) -> dict | None:
        return None


class AppController:
    """Small orchestration layer for stateful product behavior."""

    def __init__(self, config: AppConfig, state: AppState | None = None):
        self.config = config
        self.state = state or AppState.load()
        self.prompt_builder = PromptBuilder()

    @property
    def active_session(self) -> str:
        return self.state.saved_session or self.config.session.saved_session

    @property
    def active_session_name(self) -> str:
        return self.state.saved_session_name or "jcode-panel"

    def build_prompt(self, text: str, context: ActiveContext | None, include_context: bool, metadata_supported: bool = False) -> tuple[str, dict | None]:
        request = PromptRequest(text=text, context=context, include_context=include_context, metadata_supported=metadata_supported)
        return self.prompt_builder.build_text(request), self.prompt_builder.build_metadata(request)

    def record_sent_prompt(self, prompt: str) -> None:
        self.state.remember_prompt(prompt)
        self.state.save()

    def switch_session(self, session: str, name: str | None = None) -> None:
        self.state.set_saved_session(session)
        if name is not None:
            self.state.set_saved_session_name(name)
        self.config.session.saved_session = session
        self.state.save()
        self.config.save()

    def start_new_section(self, name: str | None = None) -> str:
        section_name = (name or "").strip() or f"jcode-panel {datetime.now().strftime('%Y-%m-%d %H:%M')}"
        self.state.set_saved_session("")
        self.state.set_saved_session_name(section_name)
        self.config.session.saved_session = ""
        self.state.save()
        self.config.save()
        return section_name
