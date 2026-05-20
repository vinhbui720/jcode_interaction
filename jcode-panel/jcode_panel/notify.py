from __future__ import annotations

import shutil
import subprocess


def notify(title: str, body: str = "") -> None:
    """Best-effort desktop notification. Never fails app startup."""
    if not shutil.which("notify-send"):
        return
    try:
        subprocess.Popen(["notify-send", "-a", "jcode-panel", title, body], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except Exception:
        pass
