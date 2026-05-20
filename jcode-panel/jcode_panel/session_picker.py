from __future__ import annotations

import subprocess


def list_sessions() -> list[str]:
    for args in (["jcode", "sessions", "--json"], ["jcode", "session", "list", "--json"]):
        try:
            out = subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL, timeout=2)
            import json
            data = json.loads(out)
            if isinstance(data, list):
                return [str(x.get("name", x.get("id", x))) if isinstance(x, dict) else str(x) for x in data]
        except Exception:
            pass
    return []
