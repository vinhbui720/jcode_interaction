from __future__ import annotations

from dataclasses import dataclass
import json
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Optional


@dataclass
class BrowserContext:
    title: str = ""
    url: str = ""
    selected_text: str = ""


@dataclass
class ActiveContext:
    app: str = ""
    window_title: str = ""
    browser: BrowserContext | None = None

    def summary(self) -> str:
        if self.browser and (self.browser.title or self.browser.url):
            host = self.browser.url.replace("https://", "").replace("http://", "").split("/")[0]
            bits = [self.app or "Browser", host or self.browser.title]
            if self.browser.selected_text:
                bits.append("selected text")
            return " · ".join(b for b in bits if b)
        return " · ".join(b for b in [self.app, self.window_title] if b) or "No context"

    def as_prompt_block(self) -> str:
        lines = ["[Context]"]
        if self.app:
            lines.append(f"App: {self.app}")
        if self.window_title:
            lines.append(f"Title: {self.window_title}")
        if self.browser:
            if self.browser.url:
                lines.append(f"URL: {self.browser.url}")
            if self.browser.title and self.browser.title != self.window_title:
                lines.append(f"Tab title: {self.browser.title}")
            if self.browser.selected_text:
                lines.append(f"Selected text: {self.browser.selected_text}")
        lines.append("[/Context]")
        return "\n".join(lines)


_latest_browser = BrowserContext()


def _run(args: list[str]) -> str:
    try:
        return subprocess.check_output(args, stderr=subprocess.DEVNULL, text=True, timeout=1).strip()
    except Exception:
        return ""


def capture_active_context(window_id: str = "") -> ActiveContext:
    window = window_id or _run(["xdotool", "getactivewindow"])
    title = _run(["xdotool", "getwindowname", window]) if window else ""
    app = ""
    if window:
        wm_class = _run(["xprop", "-id", window, "WM_CLASS"])
        if wm_class:
            parts = [p.strip().strip('"') for p in wm_class.split("=")[-1].split(",")]
            app = parts[-1] if parts else ""
    browser = _latest_browser if (_latest_browser.title or _latest_browser.url or _latest_browser.selected_text) else None
    return ActiveContext(app=app, window_title=title, browser=browser)


class BrowserBridge:
    def __init__(self, host: str = "127.0.0.1", port: int = 8765):
        self.host = host
        self.port = port
        self.server: Optional[ThreadingHTTPServer] = None
        self.thread: Optional[threading.Thread] = None

    def start(self) -> None:
        global _latest_browser

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):  # noqa: N802
                global _latest_browser
                length = int(self.headers.get("content-length", "0"))
                raw = self.rfile.read(length)
                try:
                    data = json.loads(raw.decode("utf-8"))
                    _latest_browser = BrowserContext(
                        title=str(data.get("title", "")),
                        url=str(data.get("url", "")),
                        selected_text=str(data.get("selectedText", "") or data.get("selected_text", "")),
                    )
                    self.send_response(204)
                    self.end_headers()
                except Exception:
                    self.send_response(400)
                    self.end_headers()

            def do_GET(self):  # noqa: N802
                body = json.dumps(_latest_browser.__dict__).encode()
                self.send_response(200)
                self.send_header("content-type", "application/json")
                self.send_header("content-length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_args):
                return

        self.server = ThreadingHTTPServer((self.host, self.port), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def stop(self) -> None:
        if self.server:
            self.server.shutdown()
