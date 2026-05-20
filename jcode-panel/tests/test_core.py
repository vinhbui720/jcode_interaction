from pathlib import Path

from jcode_panel.config import AppConfig
from jcode_panel.context import ActiveContext, BrowserContext
from jcode_panel.dropdown import ConversationBuffer
from jcode_panel.floating import CompletionState
from jcode_panel.jcode_client import parse_event
from jcode_panel.protocol import PanelEventKind, event_preview, parse_panel_event
from jcode_panel.services import AppController, PromptBuilder, PromptRequest
from jcode_panel.state import AppState
from jcode_panel.terminal import render_command


def test_default_config_roundtrip(tmp_path: Path):
    path = tmp_path / "config.toml"
    cfg = AppConfig.load(path)
    assert cfg.general.hotkey == "f8"
    cfg.general.debug = True
    cfg.save(path)
    assert AppConfig.load(path).general.debug is True


def test_context_summary_and_block():
    ctx = ActiveContext(app="Firefox", window_title="Issue", browser=BrowserContext(title="Repo", url="https://github.com/a/b", selected_text="hello"))
    assert "github.com" in ctx.summary()
    block = ctx.as_prompt_block()
    assert "Selected text: hello" in block
    assert block.startswith("[Context]")


def test_parse_structured_and_plain_events():
    event = parse_event('{"type":"status","text":"Running tests"}')
    assert event.kind == PanelEventKind.STATUS
    assert event.text == "Running tests"
    assert parse_event("hello").text == "hello"


def test_conversation_preview_debug_and_status():
    buf = ConversationBuffer(max_messages=2)
    buf.add_user("a")
    buf.add_user("b")
    buf.add_user("c")
    assert len(buf.messages) == 2
    assert "c" in buf.latest_preview(debug=True)


def test_completion_tab_cycles():
    state = CompletionState(["/a", "/b"])
    assert state.tab() == "/a"
    assert state.tab() == "/b"
    assert state.tab() == "/a"


def test_terminal_template_rendering():
    args = render_command("xterm -e sh -lc {quoted_cmd}", "jcode --resume fox")
    assert args[:4] == ["xterm", "-e", "sh", "-lc"]
    assert args[4] == "jcode --resume fox"

def test_state_roundtrip_and_prompt_dedupe(tmp_path: Path):
    path = tmp_path / "state.toml"
    state = AppState(saved_session="fox")
    state.remember_prompt("hello")
    state.remember_prompt("hello")
    state.remember_prompt("world")
    state.save(path)
    loaded = AppState.load(path)
    assert loaded.saved_session == "fox"
    assert loaded.prompt_history == ["hello", "world"]


def test_prompt_builder_context_fallback_and_metadata():
    ctx = ActiveContext(app="Firefox", window_title="Issue", browser=BrowserContext(url="https://example.com"))
    builder = PromptBuilder()
    text = builder.build_text(PromptRequest("explain", ctx, include_context=True))
    assert text.startswith("[Context]")
    assert text.endswith("explain")
    metadata_req = PromptRequest("explain", ctx, include_context=True, metadata_supported=True)
    assert builder.build_text(metadata_req) == "explain"
    metadata = builder.build_metadata(metadata_req)
    assert metadata and metadata["browser"]["url"] == "https://example.com"


def test_app_controller_active_session_prefers_state():
    cfg = AppConfig()
    cfg.session.saved_session = "config-session"
    controller = AppController(cfg, AppState(saved_session="state-session"))
    assert controller.active_session == "state-session"

def test_protocol_parses_panel_events_and_preview():
    event = parse_panel_event('{"type":"panel.progress","text":"Running tests","percent":42}')
    assert event.kind == PanelEventKind.PROGRESS
    assert event.progress == 0.42
    assert "42%" in event_preview(event)


def test_protocol_parses_completion_items_and_session():
    event = parse_panel_event('{"type":"panel.completions","items":[{"value":"/grill-me","detail":"ask"}]}')
    assert event.kind == PanelEventKind.COMPLETIONS
    assert event.completions[0].value == "/grill-me"
    session = parse_panel_event('{"type":"panel.session","session_id":"fox"}')
    assert session.session_id == "fox"

from jcode_panel.diagnostics import CheckResult, DiagnosticsReport


def test_diagnostics_report_text_and_status():
    report = DiagnosticsReport([CheckResult("a", True, "ok"), CheckResult("b", False, "bad", "fix it")])
    text = report.as_text()
    assert not report.ok
    assert "[OK] a" in text
    assert "fix: fix it" in text

from jcode_panel.hotkeys import HotkeyStatus


def test_hotkey_status_shape():
    status = HotkeyStatus(False, "nope")
    assert not status.enabled
    assert "tray menu" in status.fallback

from jcode_panel.integrations import IntegrationRegistry
from jcode_panel.updater import UpdateResult


def test_integration_registry_lists_browser_and_obsidian():
    registry = IntegrationRegistry(Path(__file__).resolve().parents[1])
    statuses = registry.list_statuses()
    names = {s.name for s in statuses}
    assert "Browser Context Extension" in names
    assert "Obsidian Context Plugin" in names


def test_update_result_shape():
    result = UpdateResult(True, "Already up to date")
    assert result.ok
    assert not result.changed

from jcode_panel.control import ControlResponse


def test_control_response_shape():
    response = ControlResponse(True, "running")
    assert response.ok
    assert response.message == "running"

from jcode_panel.gnome_shortcut import ShortcutResult


def test_shortcut_result_shape():
    result = ShortcutResult(True, "installed")
    assert result.ok
    assert result.message == "installed"


def test_protocol_extracts_common_ndjson_text_shapes():
    assert parse_panel_event('{"type":"assistant","delta":"hi"}').text == "hi"
    assert parse_panel_event('{"type":"message","content":[{"text":"a"},{"text":"b"}]}').text == "ab"
