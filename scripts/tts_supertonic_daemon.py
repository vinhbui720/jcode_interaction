from __future__ import annotations

import json
import signal
import subprocess
import sys
from pathlib import Path

from supertonic import TTS

OUT_DIR = Path("/tmp/jcode-supertonic-tts")
OUT_DIR.mkdir(parents=True, exist_ok=True)
READY = False
RUNNING = True


def _play(path: Path) -> None:
    players = (["paplay", str(path)], ["aplay", str(path)])
    for command in players:
        try:
            completed = subprocess.run(
                command,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            if completed.returncode == 0:
                return
        except FileNotFoundError:
            continue


def _on_signal(_signum: int, _frame: object) -> None:
    global RUNNING
    RUNNING = False


def main() -> int:
    global READY
    signal.signal(signal.SIGTERM, _on_signal)
    signal.signal(signal.SIGINT, _on_signal)

    tts = TTS(auto_download=True)
    style = tts.get_voice_style(voice_name="M4")
    print("READY", flush=True)
    READY = True

    for raw_line in sys.stdin:
        if not RUNNING:
            break
        line = raw_line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as error:
            print(f"ERR bad-json {error}", flush=True)
            continue
        command = payload.get("command", "speak")
        if command == "shutdown":
            print("BYE", flush=True)
            break
        if command != "speak":
            print(f"ERR bad-command {command}", flush=True)
            continue
        text = str(payload.get("text", "")).strip()
        if not text:
            print("OK empty", flush=True)
            continue
        out_file = OUT_DIR / "reply.wav"
        try:
            wav, _duration = tts.synthesize(text, voice_style=style)
            tts.save_audio(wav, str(out_file))
            _play(out_file)
            print(f"OK {out_file}", flush=True)
        except Exception as error:  # noqa: BLE001
            print(f"ERR synth {error}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
