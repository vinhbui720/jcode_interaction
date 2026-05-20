from __future__ import annotations

from dataclasses import dataclass
import os
from typing import Callable


@dataclass
class HotkeyStatus:
    enabled: bool
    message: str
    fallback: str = "Use the tray menu → Prompt, or run `jcode-panel --prompt`."


def start_hotkey_listener(hotkey: str, callback: Callable[[], None]) -> HotkeyStatus:
    """Start best-effort global hotkey listener.

    On Wayland, global key capture is often blocked. We still try because some
    sessions allow it, but return an explicit degraded-status message.
    """
    session_type = os.environ.get("XDG_SESSION_TYPE", "").lower()
    try:
        from pynput import keyboard  # type: ignore
    except Exception as exc:
        return HotkeyStatus(False, f"Global hotkey unavailable: {exc}")

    normalized = hotkey.lower()

    def on_press(key):
        name = getattr(key, "name", None) or getattr(key, "char", "")
        if str(name).lower() == normalized:
            callback()

    try:
        listener = keyboard.Listener(on_press=on_press)
        listener.daemon = True
        listener.start()
    except Exception as exc:
        return HotkeyStatus(False, f"Global hotkey failed to start: {exc}")

    if session_type == "wayland":
        return HotkeyStatus(True, "Wayland detected: F8 may be blocked by compositor. Tray Prompt still works.")
    return HotkeyStatus(True, f"Global hotkey active: {hotkey}")
