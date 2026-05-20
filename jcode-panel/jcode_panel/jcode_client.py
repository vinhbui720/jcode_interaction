from __future__ import annotations

import json
import queue
import shutil
import subprocess
import threading
import time
from typing import Callable

from .protocol import CompletionItem, ConnectionState, PanelEvent, PanelEventKind, parse_panel_event


class JcodeUnavailable(RuntimeError):
    pass


class JcodeClient:
    """Thin jcode client wrapper.

    It prefers jcode's future formal APIs when commands exist, while keeping a
    subprocess streaming fallback usable today.
    """

    def __init__(self, session_id: str = ""):
        self.session_id = session_id
        self.process: subprocess.Popen | None = None
        self.events: "queue.Queue[PanelEvent]" = queue.Queue()
        self._reader: threading.Thread | None = None
        self.state = ConnectionState.DISCONNECTED
        self.last_error = ""

    def ensure_available(self) -> None:
        if not shutil.which("jcode"):
            raise JcodeUnavailable("jcode is not installed or not on PATH")

    def start_server(self) -> None:
        self.ensure_available()
        subprocess.Popen(["jcode", "serve"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.3)

    def connect(self) -> None:
        self.ensure_available()
        # `jcode connect` is a TUI client and can exit/break pipes when driven
        # without a PTY. The panel quick-prompt path uses `jcode run` per
        # prompt until a formal stable panel API exists.
        self.state = ConnectionState.CONNECTED
        self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="jcode-panel ready"))

    def _read_stdout(self) -> None:
        assert self.process and self.process.stdout
        try:
            for line in self.process.stdout:
                self.events.put(parse_event(line.rstrip("\n")))
        except Exception as exc:
            self.last_error = str(exc)
            self.state = ConnectionState.ERROR
            self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode stream error: {exc}"))
        finally:
            if self.state == ConnectionState.CONNECTED:
                self.state = ConnectionState.DISCONNECTED
                self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="Disconnected from jcode"))

    def send(self, prompt: str, context: dict | None = None) -> None:
        self.ensure_available()
        if not prompt.strip():
            return
        threading.Thread(target=self._run_prompt, args=(prompt,), daemon=True).start()

    def _run_prompt(self, prompt: str) -> None:
        args = ["jcode", "run", "--ndjson"]
        if self.session_id:
            args += ["--resume", self.session_id]
        args.append(prompt)
        self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="Sending prompt to jcode..."))
        try:
            proc = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
            assert proc.stdout
            for line in proc.stdout:
                line = line.rstrip("\n")
                if line:
                    self.events.put(parse_event(line))
            code = proc.wait()
            if code == 0:
                self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="jcode response complete"))
            else:
                self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode run exited with code {code}"))
        except Exception as exc:
            self.last_error = str(exc)
            self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode run failed: {exc}"))

    def stream(self, on_event: Callable[[PanelEvent], None]) -> None:
        while True:
            on_event(self.events.get())

    def disconnect(self) -> None:
        if self.process and self.process.poll() is None:
            self.process.terminate()
        self.state = ConnectionState.DISCONNECTED

    def reconnect(self) -> None:
        self.state = ConnectionState.RECONNECTING
        self.disconnect()
        self.connect()

    def completions(self, prefix: str) -> list[str]:
        self.ensure_available()
        api_attempts = [
            ["jcode", "completion", "--json", prefix],
            ["jcode", "completions", "--json", prefix],
        ]
        for args in api_attempts:
            try:
                out = subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL, timeout=2)
                data = json.loads(out)
                if isinstance(data, list):
                    return [CompletionItem.from_any(x).value for x in data]
                if isinstance(data, dict) and isinstance(data.get("items"), list):
                    return [CompletionItem.from_any(x).value for x in data["items"]]
            except Exception:
                pass
        fallback = ["/help", "/resume", "/skill", "/swarm", "/memory"]
        return [x for x in fallback if x.startswith(prefix)]

    def resume_command(self, session: str) -> str:
        return f"jcode --resume {session}"


def parse_event(line: str) -> PanelEvent:
    return parse_panel_event(line)
