from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import json
import os
import socket
import threading
from typing import Callable

RUNTIME_DIR = Path(os.environ.get("XDG_RUNTIME_DIR", f"/tmp/jcode-panel-{os.getuid()}"))
SOCKET_PATH = RUNTIME_DIR / "jcode-panel.sock"


@dataclass
class ControlResponse:
    ok: bool
    message: str


def send_control(command: str, timeout: float = 1.0) -> ControlResponse:
    try:
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.settimeout(timeout)
        client.connect(str(SOCKET_PATH))
        client.sendall((json.dumps({"command": command}) + "\n").encode())
        raw = client.recv(4096).decode().strip()
        data = json.loads(raw) if raw else {}
        return ControlResponse(bool(data.get("ok", False)), str(data.get("message", "")))
    except Exception as exc:
        return ControlResponse(False, str(exc))
    finally:
        try:
            client.close()  # type: ignore[name-defined]
        except Exception:
            pass


class ControlServer:
    def __init__(self, handler: Callable[[str], ControlResponse]):
        self.handler = handler
        self.socket: socket.socket | None = None
        self.thread: threading.Thread | None = None
        self.running = False

    def start(self) -> bool:
        RUNTIME_DIR.mkdir(parents=True, exist_ok=True)
        if SOCKET_PATH.exists():
            # Stale socket or active instance. Active instances should be handled
            # before server start by send_control().
            try:
                SOCKET_PATH.unlink()
            except Exception:
                return False
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.bind(str(SOCKET_PATH))
        self.socket.listen(5)
        self.running = True
        self.thread = threading.Thread(target=self._serve, daemon=True)
        self.thread.start()
        return True

    def _serve(self) -> None:
        assert self.socket
        while self.running:
            try:
                conn, _ = self.socket.accept()
            except Exception:
                break
            threading.Thread(target=self._handle, args=(conn,), daemon=True).start()

    def _handle(self, conn: socket.socket) -> None:
        with conn:
            try:
                raw = conn.recv(4096).decode().strip()
                data = json.loads(raw) if raw else {}
                response = self.handler(str(data.get("command", "status")))
            except Exception as exc:
                response = ControlResponse(False, str(exc))
            conn.sendall((json.dumps(response.__dict__) + "\n").encode())

    def stop(self) -> None:
        self.running = False
        if self.socket:
            self.socket.close()
        try:
            SOCKET_PATH.unlink()
        except Exception:
            pass
