from __future__ import annotations

import json
import queue
import shutil
import subprocess
import threading
import time
from dataclasses import dataclass
from typing import Callable, Iterable


@dataclass
class JcodeEvent:
    kind: str
    text: str = ""
    raw: dict | None = None


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
        self.events: "queue.Queue[JcodeEvent]" = queue.Queue()
        self._reader: threading.Thread | None = None

    def ensure_available(self) -> None:
        if not shutil.which("jcode"):
            raise JcodeUnavailable("jcode is not installed or not on PATH")

    def start_server(self) -> None:
        self.ensure_available()
        subprocess.Popen(["jcode", "serve"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.3)

    def connect(self) -> None:
        self.ensure_available()
        args = ["jcode", "connect"]
        if self.session_id:
            args += ["--resume", self.session_id]
        self.process = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        self._reader = threading.Thread(target=self._read_stdout, daemon=True)
        self._reader.start()

    def _read_stdout(self) -> None:
        assert self.process and self.process.stdout
        for line in self.process.stdout:
            self.events.put(parse_event(line.rstrip("\n")))

    def send(self, prompt: str, context: dict | None = None) -> None:
        if not self.process or not self.process.stdin:
            raise JcodeUnavailable("jcode client is not connected")
        # Future structured metadata can be negotiated here. Fallback sends text.
        self.process.stdin.write(prompt + "\n")
        self.process.stdin.flush()

    def stream(self, on_event: Callable[[JcodeEvent], None]) -> None:
        while True:
            on_event(self.events.get())

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
                    return [str(x) for x in data]
                if isinstance(data, dict) and isinstance(data.get("items"), list):
                    return [str(x.get("value", x)) for x in data["items"]]
            except Exception:
                pass
        fallback = ["/help", "/resume", "/skill", "/swarm", "/memory"]
        return [x for x in fallback if x.startswith(prefix)]

    def resume_command(self, session: str) -> str:
        return f"jcode --resume {session}"


def parse_event(line: str) -> JcodeEvent:
    try:
        data = json.loads(line)
        if isinstance(data, dict):
            kind = str(data.get("type") or data.get("kind") or "message")
            text = str(data.get("text") or data.get("content") or data.get("message") or line)
            return JcodeEvent(kind=kind, text=text, raw=data)
    except Exception:
        pass
    return JcodeEvent(kind="text", text=line)
