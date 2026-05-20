from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path
import os
try:
    import tomllib  # Python 3.11+
except Exception:  # pragma: no cover - Python 3.10 fallback
    tomllib = None

try:
    import tomli_w  # type: ignore
except Exception:  # pragma: no cover
    tomli_w = None

CONFIG_HOME = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "jcode-panel"
CONFIG_PATH = CONFIG_HOME / "config.toml"
STATE_PATH = CONFIG_HOME / "state.toml"


@dataclass
class GeneralConfig:
    hotkey: str = "f8"
    terminal: str = "auto"
    terminal_template: str = ""
    autostart: bool = True
    debug: bool = False
    auto_update_on_start: bool = False


@dataclass
class SessionConfig:
    auto_resume: bool = True
    show_context_strip: bool = True
    send_context_default: bool = True
    saved_session: str = ""


@dataclass
class UIConfig:
    dropdown_max_messages: int = 20
    floating_opacity: float = 0.92


@dataclass
class VoiceConfig:
    enabled: bool = False
    hotkey: str = "alt+v"


@dataclass
class AppConfig:
    general: GeneralConfig = field(default_factory=GeneralConfig)
    session: SessionConfig = field(default_factory=SessionConfig)
    ui: UIConfig = field(default_factory=UIConfig)
    voice: VoiceConfig = field(default_factory=VoiceConfig)

    @classmethod
    def load(cls, path: Path = CONFIG_PATH) -> "AppConfig":
        cfg = cls()
        if not path.exists():
            cfg.save(path)
            return cfg
        text = path.read_text()
        data = tomllib.loads(text) if tomllib else _load_simple_toml(text)
        for section_name, section_cls in (
            ("general", GeneralConfig),
            ("session", SessionConfig),
            ("ui", UIConfig),
            ("voice", VoiceConfig),
        ):
            values = data.get(section_name, {}) or {}
            current = getattr(cfg, section_name)
            allowed = current.__dataclass_fields__.keys()
            setattr(cfg, section_name, section_cls(**{k: v for k, v in values.items() if k in allowed}))
        return cfg

    def to_dict(self) -> dict:
        return asdict(self)

    def save(self, path: Path = CONFIG_PATH) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        data = self.to_dict()
        if tomli_w:
            path.write_text(tomli_w.dumps(data))
        else:
            path.write_text(_dump_simple_toml(data))


def _dump_simple_toml(data: dict) -> str:
    lines: list[str] = []
    for section, values in data.items():
        lines.append(f"[{section}]")
        for key, value in values.items():
            if isinstance(value, bool):
                rendered = "true" if value else "false"
            elif isinstance(value, str):
                rendered = repr(value).replace("'", '"')
            else:
                rendered = str(value)
            lines.append(f"{key} = {rendered}")
        lines.append("")
    return "\n".join(lines)


def _load_simple_toml(text: str) -> dict:
    """Tiny TOML subset parser for our generated config on Python 3.10."""
    data: dict[str, dict] = {}
    current: dict | None = None
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            current = data.setdefault(line[1:-1].strip(), {})
            continue
        if current is None or "=" not in line:
            continue
        key, value = [part.strip() for part in line.split("=", 1)]
        if value.lower() in {"true", "false"}:
            parsed = value.lower() == "true"
        elif value.startswith(('"', "'")) and value.endswith(('"', "'")):
            parsed = value[1:-1]
        else:
            try:
                parsed = int(value)
            except ValueError:
                try:
                    parsed = float(value)
                except ValueError:
                    parsed = value
        current[key] = parsed
    return data
