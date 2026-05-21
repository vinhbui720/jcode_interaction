from __future__ import annotations

from dataclasses import dataclass
import os
from typing import Callable


MODIFIER_ALIASES = {
    "control": "ctrl",
    "control_l": "ctrl",
    "control_r": "ctrl",
    "ctrl_l": "ctrl",
    "ctrl_r": "ctrl",
    "shift_l": "shift",
    "shift_r": "shift",
    "alt_l": "alt",
    "alt_r": "alt",
    "option": "alt",
    "super_l": "super",
    "super_r": "super",
    "cmd": "super",
    "command": "super",
    "win": "super",
    "windows": "super",
    "esc": "escape",
    "return": "enter",
    "kp_enter": "enter",
}

MODIFIER_ORDER = ["ctrl", "alt", "shift", "super"]
MODIFIERS = set(MODIFIER_ORDER)


@dataclass
class HotkeyStatus:
    enabled: bool
    message: str
    fallback: str = "Use the tray menu → Prompt, or run `jcode-panel --prompt`."


def normalize_key_name(name: str) -> str:
    key = (name or "").strip().lower().replace(" ", "_")
    if key.startswith("key."):
        key = key[4:]
    return MODIFIER_ALIASES.get(key, key)


def key_name_from_pynput(key) -> str:
    name = getattr(key, "name", None) or getattr(key, "char", "") or ""
    return normalize_key_name(str(name))


def normalize_hotkey(hotkey: str) -> str:
    parts = [normalize_key_name(part) for part in str(hotkey or "").replace("-", "+").split("+")]
    parts = [part for part in parts if part]
    modifiers = [mod for mod in MODIFIER_ORDER if mod in parts]
    keys = [part for part in parts if part not in MODIFIERS]
    key = keys[-1] if keys else ""
    return "+".join(modifiers + ([key] if key else [])) or "f8"


def hotkey_parts(hotkey: str) -> tuple[set[str], str]:
    parts = normalize_hotkey(hotkey).split("+")
    modifiers = {part for part in parts if part in MODIFIERS}
    keys = [part for part in parts if part not in MODIFIERS]
    return modifiers, (keys[-1] if keys else "")


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

    normalized_hotkey = normalize_hotkey(hotkey)
    wanted_mods, wanted_key = hotkey_parts(normalized_hotkey)
    pressed_mods: set[str] = set()

    def on_press(key):
        key_name = key_name_from_pynput(key)
        if key_name in MODIFIERS:
            pressed_mods.add(key_name)
        elif key_name == wanted_key and wanted_mods.issubset(pressed_mods):
            callback()

    def on_release(key):
        key_name = key_name_from_pynput(key)
        if key_name in MODIFIERS:
            pressed_mods.discard(key_name)

    try:
        listener = keyboard.Listener(on_press=on_press, on_release=on_release)
        listener.daemon = True
        listener.start()
    except Exception as exc:
        return HotkeyStatus(False, f"Global hotkey failed to start: {exc}")

    if session_type == "wayland":
        return HotkeyStatus(True, "Wayland detected: F8 may be blocked by compositor. Tray Prompt still works.")
    return HotkeyStatus(True, f"Global hotkey active: {normalized_hotkey}")
