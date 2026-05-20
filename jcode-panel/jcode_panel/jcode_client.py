from __future__ import annotations

import json
import queue
import re
import shutil
import subprocess
import threading
import time
from pathlib import Path
from typing import Callable

from .protocol import CompletionItem, ConnectionState, PanelEvent, PanelEventKind, parse_panel_event


ANSI_RE = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]")


class JcodeUnavailable(RuntimeError):
    pass


class JcodeClient:
    """Panel-owned jcode client wrapper.

    For saved sections this keeps one `jcode repl --resume <session>` process
    alive and writes prompts to stdin. Brand-new sections use one `jcode run
    --ndjson` bootstrap so jcode can create and report the session id, then the
    panel switches to the long-lived REPL for all later prompts.
    """

    def __init__(self, session_id: str = ""):
        self.session_id = session_id.strip()
        self.process: subprocess.Popen | None = None
        self.events: "queue.Queue[PanelEvent]" = queue.Queue()
        self._reader: threading.Thread | None = None
        self._send_lock = threading.Lock()
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
        if self.session_id:
            self._ensure_repl()
        else:
            self.state = ConnectionState.CONNECTED
            self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="jcode-panel ready; first prompt will create a session"))

    def _repl_args(self) -> list[str]:
        args = ["jcode", "repl"]
        if self.session_id:
            args += ["--resume", self.session_id]
        return args

    def _ensure_repl(self) -> None:
        self.ensure_available()
        if self.process and self.process.poll() is None:
            return
        self.state = ConnectionState.CONNECTING
        self.process = subprocess.Popen(
            self._repl_args(),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            cwd=str(Path.home()),
        )
        self.state = ConnectionState.CONNECTED
        self._reader = threading.Thread(target=self._read_stdout, daemon=True)
        self._reader.start()
        name = self.session_id or "new"
        self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text=f"Persistent jcode client running: {name}"))

    def _read_stdout(self) -> None:
        proc = self.process
        if not proc or not proc.stdout:
            return
        try:
            for line in proc.stdout:
                line = ANSI_RE.sub("", line.rstrip("\n"))
                if not line or line.strip() == ">":
                    continue
                # Hide REPL chrome from chat, keep useful activity lines.
                if line.startswith("J-Code -") or line.startswith("Type your message") or line.startswith("Available skills:"):
                    continue
                self.events.put(parse_event(line))
        except Exception as exc:
            self.last_error = str(exc)
            self.state = ConnectionState.ERROR
            self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode stream error: {exc}"))
        finally:
            if self.process is proc and self.state == ConnectionState.CONNECTED:
                self.state = ConnectionState.DISCONNECTED
                self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="Persistent jcode client stopped"))

    def send(self, prompt: str, context: dict | None = None) -> None:
        self.ensure_available()
        prompt = prompt.strip()
        if not prompt:
            return
        threading.Thread(target=self._send_prompt, args=(prompt,), daemon=True).start()

    def _send_prompt(self, prompt: str) -> None:
        with self._send_lock:
            if self.session_id:
                self._send_to_repl(prompt)
            else:
                self._run_first_prompt(prompt)

    def _send_to_repl(self, prompt: str) -> None:
        self._ensure_repl()
        if not self.process or not self.process.stdin or self.process.poll() is not None:
            raise JcodeUnavailable("persistent jcode client is not writable")
        self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="Sending prompt to persistent jcode client..."))
        try:
            self.process.stdin.write(prompt.replace("\r", "") + "\n")
            self.process.stdin.flush()
        except Exception as exc:
            self.last_error = str(exc)
            self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode repl send failed: {exc}"))
            self.disconnect()

    def _run_first_prompt(self, prompt: str) -> None:
        """Bootstrap a new real Jcode session, then keep it alive via REPL."""
        args = ["jcode", "run", "--ndjson", prompt]
        self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="Creating new jcode session..."))
        discovered_session = ""
        try:
            proc = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1, cwd=str(Path.home()))
            assert proc.stdout
            for line in proc.stdout:
                line = line.rstrip("\n")
                if not line:
                    continue
                event = parse_event(line)
                if event.kind == PanelEventKind.SESSION and event.session_id:
                    discovered_session = event.session_id
                self.events.put(event)
            code = proc.wait()
            if code == 0:
                self.events.put(PanelEvent(kind=PanelEventKind.STATUS, text="jcode response complete"))
                if discovered_session:
                    self.set_session(discovered_session)
                    self._ensure_repl()
            else:
                self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode run exited with code {code}"))
        except Exception as exc:
            self.last_error = str(exc)
            self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode run failed: {exc}"))

    def set_session(self, session_id: str) -> None:
        session_id = session_id.strip()
        if session_id == self.session_id and (not session_id or (self.process and self.process.poll() is None)):
            return
        self.disconnect()
        self.session_id = session_id
        if self.session_id:
            try:
                self._ensure_repl()
            except Exception as exc:
                self.last_error = str(exc)
                self.events.put(PanelEvent(kind=PanelEventKind.ERROR, text=f"jcode repl start failed: {exc}"))

    def adopt_session(self, session_id: str) -> None:
        """Remember a session id discovered from events without restarting IO.

        Used while the one-shot bootstrap command is still streaming. Once it
        exits cleanly, `_run_first_prompt` starts the persistent REPL.
        """
        self.session_id = session_id.strip()

    def rename_session(self, session_id: str, name: str) -> None:
        session_id = session_id.strip()
        name = name.strip()
        if not session_id or not name:
            return
        try:
            subprocess.Popen(
                ["jcode", "session", "rename", session_id, name],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except Exception:
            pass

    def stream(self, on_event: Callable[[PanelEvent], None]) -> None:
        while True:
            on_event(self.events.get())

    def disconnect(self) -> None:
        proc = self.process
        self.process = None
        if proc and proc.poll() is None:
            try:
                if proc.stdin:
                    proc.stdin.write("quit\n")
                    proc.stdin.flush()
                proc.wait(timeout=2)
            except Exception:
                try:
                    proc.terminate()
                    proc.wait(timeout=2)
                except Exception:
                    proc.kill()
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
