from __future__ import annotations

import shutil
import subprocess
from dataclasses import dataclass


@dataclass
class ShortcutResult:
    ok: bool
    message: str


def install_f8_shortcut(command: str = "jcp", name: str = "Jcode Interaction") -> ShortcutResult:
    """Install a GNOME-native custom shortcut for Wayland-safe F8 handling."""
    if not shutil.which("gsettings"):
        return ShortcutResult(False, "gsettings not available")
    base = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/jcode-panel-prompt/"
    schema = "org.gnome.settings-daemon.plugins.media-keys"
    key = "custom-keybindings"
    binding_schema = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding"
    try:
        current = subprocess.check_output(["gsettings", "get", schema, key], text=True).strip()
        entries = [] if current == "@as []" else [x.strip().strip("'") for x in current.strip("[]").split(",") if x.strip()]
        if base not in entries:
            entries.append(base)
        rendered = "[" + ", ".join(repr(x) for x in entries) + "]"
        subprocess.check_call(["gsettings", "set", schema, key, rendered])
        path = schema + ".custom-keybinding:" + base
        subprocess.check_call(["gsettings", "set", path, "name", name])
        subprocess.check_call(["gsettings", "set", path, "command", command])
        subprocess.check_call(["gsettings", "set", path, "binding", "F8"])
        return ShortcutResult(True, "Installed GNOME F8 shortcut → jcp")
    except Exception as exc:
        return ShortcutResult(False, f"Could not install GNOME shortcut: {exc}")
