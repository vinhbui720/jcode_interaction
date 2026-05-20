from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
import os
import shutil
import subprocess

from .config import CONFIG_HOME
from .terminal import detect_terminal

LOG_DIR = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local" / "state")) / "jcode-panel"
LOG_PATH = LOG_DIR / "jcode-panel.log"


@dataclass
class CheckResult:
    name: str
    ok: bool
    message: str
    fix: str = ""


@dataclass
class DiagnosticsReport:
    checks: list[CheckResult] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return all(c.ok for c in self.checks)

    def as_text(self) -> str:
        lines = ["jcode-panel diagnostics"]
        for check in self.checks:
            status = "OK" if check.ok else "FAIL"
            lines.append(f"[{status}] {check.name}: {check.message}")
            if check.fix and not check.ok:
                lines.append(f"      fix: {check.fix}")
        return "\n".join(lines)


def append_log(message: str) -> None:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a", encoding="utf-8") as f:
        f.write(message.rstrip() + "\n")


def run_diagnostics() -> DiagnosticsReport:
    report = DiagnosticsReport()
    report.checks.append(CheckResult(
        "python", True, "Python runtime is available"
    ))
    report.checks.append(CheckResult(
        "jcode", shutil.which("jcode") is not None,
        shutil.which("jcode") or "jcode not found on PATH",
        "Install jcode and ensure it is on PATH, then run `jcode login` if needed.",
    ))
    terminal = detect_terminal()
    report.checks.append(CheckResult(
        "terminal", terminal is not None,
        terminal.name if terminal else "No supported terminal emulator detected",
        "Install gnome-terminal, wezterm, kitty, alacritty, or configure a custom template.",
    ))
    session_type = os.environ.get("XDG_SESSION_TYPE", "unknown")
    report.checks.append(CheckResult(
        "session", session_type.lower() != "wayland",
        f"XDG_SESSION_TYPE={session_type}",
        "v1 is X11-first. Wayland may limit global hotkey/window context.",
    ))
    report.checks.append(CheckResult(
        "config_dir", CONFIG_HOME.parent.exists(), f"Config base: {CONFIG_HOME.parent}",
    ))
    try:
        import gi  # type: ignore
        gi.require_version("Gtk", "3.0")
        gtk_ok = True
        gtk_msg = "GTK 3 available"
    except Exception as exc:
        gtk_ok = False
        gtk_msg = str(exc)
    report.checks.append(CheckResult(
        "gtk", gtk_ok, gtk_msg,
        "sudo apt install python3-gi gir1.2-appindicator3-0.1 gir1.2-gtk-3.0",
    ))
    try:
        import pynput  # type: ignore  # noqa: F401
        hotkey_ok = True
        hotkey_msg = "pynput available"
    except Exception as exc:
        hotkey_ok = False
        hotkey_msg = str(exc)
    report.checks.append(CheckResult(
        "hotkey", hotkey_ok, hotkey_msg,
        "python3 -m pip install --user pynput",
    ))
    return report


def check_jcode_login_hint() -> CheckResult:
    if not shutil.which("jcode"):
        return CheckResult("jcode_login", False, "jcode missing")
    try:
        out = subprocess.check_output(["jcode", "--version"], stderr=subprocess.STDOUT, text=True, timeout=2)
        return CheckResult("jcode_version", True, out.strip() or "jcode responded")
    except Exception as exc:
        return CheckResult("jcode_version", False, str(exc), "Run `jcode` in terminal and complete setup/login.")
