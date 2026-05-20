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
        text = request.text.strip()
        if not text:
            return ""
        if not request.include_context or not request.context or request.metadata_supported:
            return text
        return request.context.as_prompt_block() + "\n\n" + text

    def build_metadata(self, request: PromptRequest) -> dict | None:
        if not request.include_context or not request.context or not request.metadata_supported:
            return None
        ctx = request.context
        browser = ctx.browser
        return {
            "app": ctx.app,
            "window_title": ctx.window_title,
            "selected_text": ctx.selected_text,
            "clipboard_text": ctx.clipboard_text,
            "browser": None if not browser else {
                "title": browser.title,
                "url": browser.url,
                "selected_text": browser.selected_text,
            },
        }


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
