from __future__ import annotations

import sys
from pathlib import Path

from supertonic import TTS


def main() -> int:
    text = " ".join(sys.argv[1:]).strip()
    if not text:
        print("missing text", file=sys.stderr)
        return 2
    out_dir = Path("/tmp/jcode-supertonic-tts")
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / "reply.wav"

    tts = TTS(auto_download=True)
    style = tts.get_voice_style(voice_name="M4")
    wav, _duration = tts.synthesize(text, voice_style=style)
    tts.save_audio(wav, str(out_file))
    print(out_file)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
