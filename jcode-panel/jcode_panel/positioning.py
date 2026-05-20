from __future__ import annotations

import re
import subprocess


def parse_xdotool_mouselocation(output: str) -> tuple[int | None, int | None]:
    x, y, _window = parse_xdotool_mouselocation_full(output)
    return x, y


def parse_xdotool_mouselocation_full(output: str) -> tuple[int | None, int | None, str]:
    match = re.search(r"x:(-?\d+)\s+y:(-?\d+)", output)
    if not match:
        return None, None, ""
    window_match = re.search(r"window:([^\s]+)", output)
    return int(match.group(1)), int(match.group(2)), (window_match.group(1) if window_match else "")


def xdotool_mouse_position() -> tuple[int | None, int | None]:
    x, y, _window = xdotool_mouse_position_full()
    return x, y


def xdotool_mouse_position_full() -> tuple[int | None, int | None, str]:
    try:
        out = subprocess.check_output(["xdotool", "getmouselocation"], text=True, stderr=subprocess.DEVNULL, timeout=0.25)
        return parse_xdotool_mouselocation_full(out)
    except Exception:
        return None, None, ""
