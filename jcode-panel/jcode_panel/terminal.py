from __future__ import annotations

import shlex
import shutil
import subprocess
from dataclasses import dataclass


@dataclass(frozen=True)
class TerminalAdapter:
    name: str
    template: str


ADAPTERS = [
    TerminalAdapter("gnome-terminal", "gnome-terminal -- bash -lc {quoted_cmd}"),
    TerminalAdapter("wezterm", "wezterm start -- bash -lc {quoted_cmd}"),
    TerminalAdapter("kitty", "kitty sh -lc {quoted_cmd}"),
    TerminalAdapter("alacritty", "alacritty -e sh -lc {quoted_cmd}"),
    TerminalAdapter("xterm", "xterm -e sh -lc {quoted_cmd}"),
]


def detect_terminal(preferred: str = "auto") -> TerminalAdapter | None:
    if preferred and preferred != "auto":
        for adapter in ADAPTERS:
            if adapter.name == preferred and shutil.which(adapter.name):
                return adapter
        if shutil.which(preferred):
            return TerminalAdapter(preferred, f"{preferred} -e sh -lc {{quoted_cmd}}")
    for adapter in ADAPTERS:
        if shutil.which(adapter.name):
            return adapter
    return None


def render_command(template: str, command: str) -> list[str]:
    rendered = template.format(cmd=command, quoted_cmd=shlex.quote(command))
    return shlex.split(rendered)


def launch(command: str, preferred: str = "auto", template: str = "") -> subprocess.Popen:
    adapter = TerminalAdapter("custom", template) if template else detect_terminal(preferred)
    if not adapter:
        raise RuntimeError("No supported terminal emulator found")
    return subprocess.Popen(render_command(adapter.template, command))
