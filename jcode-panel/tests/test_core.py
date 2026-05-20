from pathlib import Path
import os

from jcode_panel.config import AppConfig
from jcode_panel.context import ActiveContext, BrowserContext
from jcode_panel.dropdown import ConversationBuffer
from jcode_panel.floating import CompletionState
from jcode_panel.jcode_client import parse_event
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
    assert event.kind == "status"
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
