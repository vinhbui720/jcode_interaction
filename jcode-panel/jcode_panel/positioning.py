from __future__ import annotations

import re
import subprocess


def parse_xdotool_mouselocation(output: str) -> tuple[int | None, int | None]:
    match = re.search(r"x:(-?\d+)\s+y:(-?\d+)", output)
    if not match:
        return None, None
    return int(match.group(1)), int(match.group(2))


def xdotool_mouse_position() -> tuple[int | None, int | None]:
    try:
        out = subprocess.check_output(["xdotool", "getmouselocation"], text=True, stderr=subprocess.DEVNULL, timeout=1)
        return parse_xdotool_mouselocation(out)
    except Exception:
        return None, None
