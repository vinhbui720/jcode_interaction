from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from .config import AppConfig
from .context import BrowserBridge, capture_active_context
from .services import AppController
from .dropdown import ConversationBuffer
from .diagnostics import append_log, run_diagnostics
from .jcode_client import JcodeClient, JcodeUnavailable
from .terminal import launch


def smoke() -> int:
    cfg = AppConfig.load(Path(os.environ.get("JCODE_PANEL_CONFIG", "/tmp/jcode-panel-smoke.toml")))
    ctx = capture_active_context()
    buf = ConversationBuffer(max_messages=cfg.ui.dropdown_max_messages)
    buf.add_user("hello")
    assert cfg.general.hotkey == "f8"
    assert "hello" in buf.latest_preview(debug=True)
    assert ctx.summary()
    return 0


def run_headless_once(prompt: str) -> int:
    cfg = AppConfig.load()
    controller = AppController(cfg)
    ctx = capture_active_context()
    client = JcodeClient(controller.active_session)
    try:
        append_log("Sending one-shot prompt")
        client.start_server()
        client.connect()
        payload, metadata = controller.build_prompt(prompt, ctx, cfg.session.send_context_default)
        client.send(payload, metadata)
        controller.record_sent_prompt(prompt)
        return 0
    except JcodeUnavailable as exc:
        append_log(f"jcode unavailable: {exc}")
        print(f"jcode unavailable: {exc}", file=sys.stderr)
        return 2


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--smoke", action="store_true", help="run non-GUI smoke validation")
    parser.add_argument("--send", metavar="PROMPT", help="send one prompt without opening GUI")
    parser.add_argument("--diagnose", action="store_true", help="print launch diagnostics")
    parser.add_argument("--open-terminal", metavar="SESSION", help="open a session in configured terminal")
    parser.add_argument("--prompt", action="store_true", help="open GUI directly to floating prompt")
    args = parser.parse_args(argv)

    if args.smoke:
        return smoke()
    if args.diagnose:
        report = run_diagnostics()
        print(report.as_text())
        return 0 if report.ok else 1
    if args.send:
        return run_headless_once(args.send)
    if args.open_terminal:
        cfg = AppConfig.load()
        launch(f"jcode --resume {args.open_terminal}", cfg.general.terminal, cfg.general.terminal_template)
        return 0

    # Import GTK lazily so tests/headless mode work without system packages.
    try:
        from .gtk_app import run_gtk_app
    except Exception as exc:
        print("GTK UI unavailable. Install python3-gi and AppIndicator3 dependencies.", file=sys.stderr)
        print(str(exc), file=sys.stderr)
        return 2
    return run_gtk_app(open_prompt=args.prompt)


if __name__ == "__main__":
    raise SystemExit(main())
