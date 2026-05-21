from pathlib import Path

from jcode_panel.config import AppConfig
from jcode_panel.context import ActiveContext, BrowserContext, capture_active_context
from jcode_panel.dropdown import ConversationBuffer
from jcode_panel.floating import CompletionState
from jcode_panel.jcode_client import JcodeClient, parse_event
from jcode_panel.protocol import PanelEventKind, activity_is_terminal, activity_label, activity_state, event_preview, parse_panel_event
from jcode_panel.services import AppController, PromptBuilder, PromptRequest
from jcode_panel.state import AppState
from jcode_panel.terminal import launch, render_command


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
    plain = parse_event("hello")
    assert plain.kind == PanelEventKind.MESSAGE
    assert plain.text == "hello"


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


def test_terminal_launch_defaults_to_home_cwd(monkeypatch):
    calls = []

    class FakeProcess:
        def wait(self):
            return 0

    def fake_popen(args, cwd=None):
        calls.append((args, cwd))
        return FakeProcess()

    monkeypatch.setattr("jcode_panel.terminal.subprocess.Popen", fake_popen)

    launch("jcode --resume fox", template="xterm -e sh -lc {quoted_cmd}")

    assert calls[0][1] == str(Path.home())

def test_state_roundtrip_and_prompt_dedupe(tmp_path: Path):
    path = tmp_path / "state.toml"
    state = AppState(saved_session="fox", saved_session_name="Panel Fox")
    state.remember_prompt("hello")
    state.remember_prompt("hello")
    state.remember_prompt("world")
    state.save(path)
    loaded = AppState.load(path)
    assert loaded.saved_session == "fox"
    assert loaded.saved_session_name == "Panel Fox"
    assert loaded.prompt_history == ["hello", "world"]


def test_prompt_builder_sends_direct_text_without_context_or_metadata():
    ctx = ActiveContext(
        app="Firefox",
        window_title="Issue",
        browser=BrowserContext(url="https://example.com"),
        selected_text="marked",
        clipboard_text="copied",
    )
    builder = PromptBuilder()
    request = PromptRequest(" explain ", ctx, include_context=True, metadata_supported=True)
    assert builder.build_text(request) == "explain"
    assert builder.build_metadata(request) is None


def test_app_controller_active_session_prefers_state():
    cfg = AppConfig()
    cfg.session.saved_session = "config-session"
    controller = AppController(cfg, AppState(saved_session="state-session"))
    assert controller.active_session == "state-session"


def test_app_controller_section_switch_and_new_section():
    cfg = AppConfig()
    controller = AppController(cfg, AppState(saved_session="old", saved_session_name="Old"))
    controller.switch_session("new-session", "New Name")
    assert controller.active_session == "new-session"
    assert controller.active_session_name == "New Name"
    assert cfg.session.saved_session == "new-session"
    section_name = controller.start_new_section("Fresh Panel")
    assert section_name == "Fresh Panel"
    assert controller.active_session == ""
    assert controller.active_session_name == "Fresh Panel"

def test_protocol_parses_panel_events_and_preview():
    event = parse_panel_event('{"type":"panel.progress","text":"Running tests","percent":42}')
    assert event.kind == PanelEventKind.PROGRESS
    assert event.progress == 0.42
    assert "42%" in event_preview(event)


def test_protocol_activity_helpers_prefer_command_and_state():
    event = parse_panel_event('{"type":"tool_start","tool_name":"bash","command":"pytest -q","state":"running"}')
    assert event.kind == PanelEventKind.TOOL
    assert activity_label(event.raw, event.text) == "pytest -q"
    assert activity_state(event.raw, event.text) == "running"
    assert not activity_is_terminal(event)


def test_protocol_activity_helpers_detect_terminal_events():
    event = parse_panel_event('{"type":"tool_end","tool_name":"read","status":"completed"}')
    assert event.kind == PanelEventKind.TOOL
    assert activity_label(event.raw, event.text) == "read"
    assert activity_is_terminal(event)


def test_protocol_backend_chat_status_drives_live_activity():
    event = parse_panel_event(
        '{"type":"backend/chat/status","activity":{"tool_name":"bash","command":"pytest -q","state":"running","active":true}}'
    )
    assert event.kind == PanelEventKind.STATUS
    assert activity_label(event.raw, event.text) == "pytest -q"
    assert activity_state(event.raw, event.text) == "running"
    assert not activity_is_terminal(event)


def test_protocol_backend_chat_status_can_finish_activity():
    event = parse_panel_event(
        '{"type":"persistent-section/status","activity":{"tool_name":"bash","command":"pytest -q","state":"completed","active":false}}'
    )
    assert event.kind == PanelEventKind.STATUS
    assert activity_label(event.raw, event.text) == "pytest -q"
    assert activity_is_terminal(event)


def test_protocol_backend_chat_status_current_drives_live_activity():
    event = parse_panel_event(
        '{"type":"backend/chat/status","current":{"tool_name":"read","target":"README.md","state":"running","active":true}}'
    )
    assert event.kind == PanelEventKind.STATUS
    assert activity_label(event.raw, event.text) == "read"
    assert activity_state(event.raw, event.text) == "running"
    assert not activity_is_terminal(event)


def test_protocol_backend_chat_status_current_can_finish_activity():
    event = parse_panel_event(
        '{"type":"backend/chat/status","current":{"tool_name":"read","target":"README.md","state":"completed","active":false}}'
    )
    assert event.kind == PanelEventKind.STATUS
    assert activity_label(event.raw, event.text) == "read"
    assert activity_state(event.raw, event.text) == "completed"
    assert activity_is_terminal(event)


def test_protocol_backend_chat_status_prefers_current_over_activity():
    event = parse_panel_event(
        '{"type":"backend/chat/status",'
        '"activity":{"tool_name":"bash","command":"stale pytest","state":"running","active":true},'
        '"current":{"tool_name":"read","target":"README.md","state":"completed","active":false}}'
    )
    assert activity_label(event.raw, event.text) == "read"
    assert activity_state(event.raw, event.text) == "completed"
    assert activity_is_terminal(event)


def test_protocol_backend_chat_status_preview_uses_current_when_text_empty():
    event = parse_panel_event(
        '{"type":"backend/chat/status","current":{"tool_name":"bash","command":"pytest -q","state":"running","active":true}}'
    )
    assert event_preview(event) == "pytest -q"


def test_conversation_status_uses_current_preview_when_text_empty():
    buf = ConversationBuffer(max_messages=10)
    buf.add_event(parse_panel_event(
        '{"type":"backend/chat/status","current":{"tool_name":"bash","command":"pytest -q","state":"running","active":true}}'
    ))

    assert buf.messages == [("status", "pytest -q")]


def test_conversation_transcription_becomes_user_turn():
    buf = ConversationBuffer(max_messages=10)
    buf.add_user("first")
    buf.add_event(parse_panel_event('{"type":"status","text":"[transcription] HI"}'))

    assert buf.messages == [("You", "first"), ("You", "HI")]


def test_conversation_ignores_noisy_sending_status():
    buf = ConversationBuffer(max_messages=10)
    buf.add_user("hello")
    buf.add_event(parse_panel_event('{"type":"status","text":"Sending prompt to persistent jcode client..."}'))

    assert buf.messages == [("You", "hello")]


def test_protocol_backend_chat_status_extracts_feedback_text():
    event = parse_panel_event(
        '{"type":"backend/chat/status","current":{"command":"pytest -q","state":"running","active":true},"feedback":"Running regression tests"}'
    )
    assert event.kind == PanelEventKind.STATUS
    assert event.text == "Running regression tests"
    assert activity_label(event.raw, event.text) == "pytest -q"


def test_protocol_backend_chat_status_extracts_answer_text():
    event = parse_panel_event('{"type":"persistent-section/status","answer":"Done with panel changes"}')
    assert event.kind == PanelEventKind.STATUS
    assert event.text == "Done with panel changes"


def test_protocol_backend_chat_status_nested_status_current():
    event = parse_panel_event(
        '{"type":"backend/chat/status","status":{"current":{"command":"pytest -q","state":"running","active":true}}}'
    )
    assert activity_label(event.raw, event.text) == "pytest -q"
    assert activity_state(event.raw, event.text) == "running"
    assert not activity_is_terminal(event)


def test_protocol_activity_helpers_keep_streaming_message_active():
    delta = parse_panel_event('{"type":"text_delta","text":"a"}')
    assert delta.kind == PanelEventKind.MESSAGE
    assert not activity_is_terminal(delta)
    done = parse_panel_event('{"type":"done","text":"abc"}')
    assert activity_is_terminal(done)
    end = parse_panel_event('{"type":"message_end"}')
    assert activity_is_terminal(end)


def test_jcode_client_repl_args_and_adopt_session():
    client = JcodeClient("panel-session")
    assert client._repl_args() == ["jcode", "repl", "--resume", "panel-session"]
    client.adopt_session("new-session")
    assert client.session_id == "new-session"


def test_jcode_client_model_repl_args_and_slash_fallback(monkeypatch):
    client = JcodeClient("panel-session", model="gpt-5.5")
    assert client._repl_args() == ["jcode", "-m", "gpt-5.5", "repl", "--resume", "panel-session"]

    def fake_check_output(*_args, **_kwargs):
        raise RuntimeError("no completion api")

    monkeypatch.setattr("jcode_panel.jcode_client.subprocess.check_output", fake_check_output)
    completions = client.completions("/u")
    assert "/usage" in completions
    assert "/ustage" in completions


def test_jcode_client_new_section_waits_for_bootstrap_session():
    client = JcodeClient("")
    assert client._repl_args() == ["jcode", "repl"]
    client.connect = lambda: None  # shape-only regression: no saved session means no repl args are used yet


def test_jcode_client_send_bootstraps_new_section(monkeypatch):
    client = JcodeClient("")
    calls = []

    def fake_run_first_prompt(_self, prompt: str):
        calls.append(prompt)

    def fail_send_to_repl(_self, prompt: str):
        raise AssertionError(f"unexpected repl send before session exists: {prompt}")

    monkeypatch.setattr("jcode_panel.jcode_client.JcodeClient._run_first_prompt", fake_run_first_prompt)
    monkeypatch.setattr("jcode_panel.jcode_client.JcodeClient._send_to_repl", fail_send_to_repl)

    client._send_prompt("hello")

    assert calls == ["hello"]


def test_jcode_client_set_session_restarts_dead_same_session(monkeypatch):
    client = JcodeClient("saved")
    calls = []

    class DeadProcess:
        def poll(self):
            return 1

    def fake_disconnect(_self):
        calls.append("disconnect")

    def fake_ensure_repl(_self):
        calls.append("ensure")

    client.process = DeadProcess()  # type: ignore[assignment]
    monkeypatch.setattr("jcode_panel.jcode_client.JcodeClient.disconnect", fake_disconnect)
    monkeypatch.setattr("jcode_panel.jcode_client.JcodeClient._ensure_repl", fake_ensure_repl)

    client.set_session("saved")

    assert calls == ["disconnect", "ensure"]


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


def test_conversation_coalesces_text_delta_stream():
    buf = ConversationBuffer(max_messages=10)
    buf.add_user("hello")
    buf.add_event(parse_panel_event('{"type":"connection_phase","phase":"streaming"}'))
    buf.add_event(parse_panel_event('{"type":"text_delta","text":"hello"}'))
    buf.add_event(parse_panel_event('{"type":"text_delta","text":" world"}'))
    buf.add_event(parse_panel_event('{"type":"message_end"}'))
    buf.add_event(parse_panel_event('{"type":"done","text":"hello world"}'))
    assert buf.messages == [("You", "hello"), ("jcode", "hello world")]

from jcode_panel.positioning import parse_xdotool_mouselocation


def test_parse_xdotool_mouselocation():
    assert parse_xdotool_mouselocation('x:2657 y:50 screen:0 window:8388629') == (2657, 50)
    assert parse_xdotool_mouselocation('bad') == (None, None)

from jcode_panel.positioning import parse_xdotool_mouselocation_full


def test_parse_xdotool_mouselocation_full_window():
    assert parse_xdotool_mouselocation_full('x:2657 y:50 screen:0 window:8388629') == (2657, 50, '8388629')


def test_context_prompt_block_includes_selection_and_clipboard():
    ctx = ActiveContext(app="Files", window_title="Pictures", selected_text="marked", clipboard_text="file:///tmp/a.png")
    block = ctx.as_prompt_block()
    assert "Selected text: marked" in block
    assert "Clipboard: file:///tmp/a.png" in block


def test_capture_active_context_reads_selection_and_clipboard(monkeypatch):
    responses = {
        ("xdotool", "getwindowname", "123"): "Doc.pdf",
        ("xprop", "-id", "123", "WM_CLASS"): 'WM_CLASS(STRING) = "evince", "Evince"',
        ("xclip", "-o", "-selection", "primary"): "highlighted text",
        ("xclip", "-o", "-selection", "clipboard"): "copied text",
    }

    def fake_run(args):
        return responses.get(tuple(args), "")

    monkeypatch.setattr("jcode_panel.context._run", fake_run)

    ctx = capture_active_context("123")

    assert ctx.app == "Evince"
    assert ctx.window_title == "Doc.pdf"
    assert ctx.selected_text == "highlighted text"
    assert ctx.clipboard_text == "copied text"


def test_capture_active_context_filters_gjs_shell_artifact_and_dm_clipboard(monkeypatch):
    responses = {
        ("xdotool", "getwindowname", "123"): "@!0,0;BDHF",
        ("xprop", "-id", "123", "WM_CLASS"): 'WM_CLASS(STRING) = "gjs", "Gjs"',
        ("xclip", "-o", "-selection", "clipboard"): "✉ DM from squid",
    }

    def fake_run(args):
        return responses.get(tuple(args), "")

    monkeypatch.setattr("jcode_panel.context._run", fake_run)

    ctx = capture_active_context("123")

    assert ctx.app == ""
    assert ctx.window_title == ""
    assert ctx.clipboard_text == ""


def test_ambient_key_ignored_when_entry_has_focus():
    from types import SimpleNamespace
    from jcode_panel.gtk_app import PanelApp

    appended = []

    class Entry:
        def has_focus(self):
            return True

    app = SimpleNamespace(
        _ambient_shift=False,
        _ambient_ctrl=False,
        _ambient_alt=False,
        floating=SimpleNamespace(
            get_visible=lambda: True,
            suppress_listener=object(),
            entry=Entry(),
            append_text=appended.append,
            submit=lambda: appended.append("<submit>"),
            hide=lambda: appended.append("<hide>"),
            backspace=lambda: appended.append("<backspace>"),
        ),
    )

    key = SimpleNamespace(name="", char="a")
    PanelApp._route_ambient_key(app, key, True, force=True)

    assert appended == []


def test_ambient_key_routes_when_entry_not_focused():
    from types import SimpleNamespace
    from jcode_panel.gtk_app import PanelApp

    appended = []

    class Entry:
        def has_focus(self):
            return False

    app = SimpleNamespace(
        _ambient_shift=False,
        _ambient_ctrl=False,
        _ambient_alt=False,
        floating=SimpleNamespace(
            get_visible=lambda: True,
            suppress_listener=object(),
            entry=Entry(),
            append_text=appended.append,
            submit=lambda: appended.append("<submit>"),
            hide=lambda: appended.append("<hide>"),
            backspace=lambda: appended.append("<backspace>"),
        ),
    )

    key = SimpleNamespace(name="", char="a")
    PanelApp._route_ambient_key(app, key, True, force=True)

    assert appended == ["a"]


def test_markdown_to_pango_renders_safe_colored_subset():
    from jcode_panel.gtk_app import markdown_to_pango

    markup = markdown_to_pango("# Title\n- **done** with `cmd` and <unsafe>")

    assert "Title" in markup
    assert "foreground" in markup
    assert "weight=\"bold\"" in markup
    assert "font_family=\"monospace\"" in markup
    assert "&lt;unsafe&gt;" in markup


def test_format_stream_lines_keeps_recent_lines_and_wraps_long_stream():
    from jcode_panel.gtk_app import format_stream_lines

    assert format_stream_lines("a\nb\nc", max_lines=2) == "b\nc"
    wrapped = format_stream_lines("x " * 260, max_lines=3)
    assert "\n" in wrapped
    assert len(wrapped.splitlines()) <= 3


def test_token_notice_from_raw_supports_common_usage_shapes():
    from jcode_panel.gtk_app import token_notice_from_raw

    assert token_notice_from_raw({"usage": {"input_tokens": 12, "output_tokens": 34}}) == "tokens: in 12, out 34"
    assert token_notice_from_raw({"total_tokens": 46}) == "tokens: total 46"


def test_event_notice_text_separates_context_and_tokens():
    from types import SimpleNamespace
    from jcode_panel.gtk_app import PanelApp
    from jcode_panel.protocol import PanelEvent, PanelEventKind

    app = SimpleNamespace()
    event = PanelEvent(
        kind=PanelEventKind.STATUS,
        text="running",
        raw={
            "state": "running",
            "tool_name": "bash",
            "context": {"app": "Firefox", "url": "https://example.com"},
            "usage": {"input_tokens": 10, "output_tokens": 20},
        },
    )

    notice = PanelApp._event_notice_text(app, event)

    assert "running" in notice
    assert "bash" in notice
    assert "context: Firefox" in notice
    assert "tokens: in 10, out 20" in notice


def test_hotkey_normalization_and_parts():
    from jcode_panel.hotkeys import hotkey_parts, normalize_hotkey, normalize_key_name

    assert normalize_key_name("Control_L") == "ctrl"
    assert normalize_key_name("Return") == "enter"
    assert normalize_hotkey("Alt+Control+J") == "ctrl+alt+j"
    assert normalize_hotkey("shift-super-space") == "shift+super+space"
    mods, key = hotkey_parts("ctrl+alt+j")
    assert mods == {"ctrl", "alt"}
    assert key == "j"


def test_parse_screenshot_command_modes_and_prompt():
    from jcode_panel.gtk_app import parse_screenshot_command

    assert parse_screenshot_command("") == ("ask", "Analyze this screenshot.")
    assert parse_screenshot_command("full what changed?") == ("full", "what changed?")
    assert parse_screenshot_command("area explain this") == ("area", "explain this")
    assert parse_screenshot_command("region") == ("area", "Analyze this screenshot.")
    assert parse_screenshot_command("what is on screen?") == ("ask", "what is on screen?")


def test_screenshot_command_lists_distinguish_area_and_full():
    from pathlib import Path
    from types import SimpleNamespace
    from jcode_panel.gtk_app import PanelApp

    app = SimpleNamespace()
    area = PanelApp._screenshot_commands(app, Path("/tmp/a.png"), "area")
    full = PanelApp._screenshot_commands(app, Path("/tmp/f.png"), "full")

    assert ["gnome-screenshot", "-a", "-f", "/tmp/a.png"] in area
    assert ["gnome-screenshot", "-f", "/tmp/f.png"] in full
    assert any(cmd[:2] == ["grim", "-g"] for cmd in area)
    assert ["grim", "/tmp/f.png"] in full


def test_default_config_has_screenshot_hotkey():
    from jcode_panel.config import AppConfig
    from jcode_panel.hotkeys import normalize_hotkey

    cfg = AppConfig()
    assert normalize_hotkey(cfg.general.screenshot_hotkey) == "ctrl+shift+s"


def test_screenshot_tag_format_and_delete_regex():
    from jcode_panel.gtk_app import SCREENSHOT_TAG_RE, screenshot_tag

    tag = screenshot_tag("/tmp/jcode-panel-screenshots/a.png")
    assert tag == "[screenshot:/tmp/jcode-panel-screenshots/a.png]"
    text = "hello " + tag + " "
    match = SCREENSHOT_TAG_RE.search(text)
    assert match
    assert text[:match.start()] == "hello "


def test_pic_tag_expands_to_screenshot_paths():
    from types import SimpleNamespace
    from jcode_panel.gtk_app import PanelApp, pic_tag

    app = SimpleNamespace(pending_screenshots=["/tmp/a.png", "/tmp/b.png"])
    text = f"compare {pic_tag(1)} and {pic_tag(2)}"

    expanded = PanelApp._expand_screenshot_chips(app, text)

    assert "Screenshot file: /tmp/a.png" in expanded
    assert "Screenshot file: /tmp/b.png" in expanded
    assert "[pic" not in expanded
