from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import json
import re

CONTEXT_DIR = Path.home() / ".local" / "state" / "jcode-panel" / "contexts"
VSCODE_CONTEXT_PATH = CONTEXT_DIR / "vscode.json"
OBSIDIAN_CONTEXT_PATH = CONTEXT_DIR / "obsidian.json"

INTERACTION_TAG_RE = re.compile(r"(?<![\w\[])@(vscode|obsidian)\b", re.IGNORECASE)
INTERACTION_CHIP_RE = re.compile(r"\[(vscode|obsidian)\]", re.IGNORECASE)


@dataclass
class InteractionContext:
    source: str
    title: str
    body: str


class InteractionContextError(RuntimeError):
    pass


def normalize_interaction_tags(text: str) -> str:
    """Turn @vscode/@obsidian mentions into lightweight editable chips."""
    return INTERACTION_TAG_RE.sub(lambda m: f"[{m.group(1).lower()}]", text)


def interaction_sources(text: str) -> list[str]:
    return [m.group(1).lower() for m in INTERACTION_CHIP_RE.finditer(text)]


def expand_interaction_chips(text: str) -> str:
    """Expand every interaction chip occurrence into an on-demand context block."""
    sources = interaction_sources(text)
    if not sources:
        return text
    prompt_text = INTERACTION_CHIP_RE.sub("", text)
    prompt_text = re.sub(r"[ \t]{2,}", " ", prompt_text).strip()
    blocks = [read_interaction_context(source).body for source in sources]
    return "\n\n".join([*blocks, "User prompt:\n" + prompt_text]).strip()


def read_interaction_context(source: str) -> InteractionContext:
    source = source.lower().strip()
    if source == "vscode":
        return _read_vscode_context()
    if source == "obsidian":
        return _read_obsidian_context()
    raise InteractionContextError(f"Unknown interaction source: {source}")


def _read_vscode_context() -> InteractionContext:
    data = _read_json_context(VSCODE_CONTEXT_PATH, "VS Code is not open or no active file found")
    file_path = str(data.get("file") or "").strip()
    if not file_path:
        raise InteractionContextError("VS Code is not open or no active file found")
    path = Path(file_path)
    line = _safe_int(data.get("line"), 0)
    selection = str(data.get("selection") or "").strip()
    workspace = str(data.get("workspaceRoot") or "").strip()
    language = str(data.get("languageId") or "").strip()
    if not path.exists():
        body = f"Context: vscode\nfile: {file_path}\nline: {line or 'unknown'}\nstatus: file path reported by VS Code but not readable from this machine"
        return InteractionContext("vscode", file_path, body)
    snippet = _slice_file(path, line or 1, before=80, after=80)
    symbols = _nearby_symbol_hints(path, line or 1)
    parts = [
        "Context: vscode",
        f"file: {file_path}",
        f"line: {line or 'unknown'}",
    ]
    if workspace:
        parts.append(f"workspace: {workspace}")
    if language:
        parts.append(f"language: {language}")
    if selection:
        parts.extend(["selection:", _fence(selection, language)])
    if symbols:
        parts.extend(["nearby symbols:", symbols])
    parts.extend(["surrounding code:", _fence(snippet, language)])
    return InteractionContext("vscode", file_path, "\n".join(parts))


def _read_obsidian_context() -> InteractionContext:
    data = _read_json_context(OBSIDIAN_CONTEXT_PATH, "Obsidian is not open or no active note found")
    path = str(data.get("path") or data.get("file") or "").strip()
    title = str(data.get("title") or Path(path).name or "Obsidian").strip()
    line = _safe_int(data.get("line"), 0)
    selection = str(data.get("selection") or "").strip()
    text = str(data.get("text") or "").strip()
    parts = ["Context: obsidian", f"note: {title}"]
    if path:
        parts.append(f"path: {path}")
    if line:
        parts.append(f"line: {line}")
    if selection:
        parts.extend(["selection:", _fence(selection, "markdown")])
    elif text:
        parts.extend(["active note excerpt:", _fence(text[:12000], "markdown")])
    else:
        raise InteractionContextError("Obsidian is not open or no active note found")
    return InteractionContext("obsidian", title, "\n".join(parts))


def _read_json_context(path: Path, missing_message: str) -> dict:
    try:
        if not path.exists():
            raise InteractionContextError(missing_message)
        return json.loads(path.read_text())
    except InteractionContextError:
        raise
    except Exception as exc:
        raise InteractionContextError(f"{missing_message}: {exc}") from exc


def _slice_file(path: Path, line: int, before: int, after: int) -> str:
    lines = path.read_text(errors="replace").splitlines()
    start = max(1, line - before)
    end = min(len(lines), line + after)
    width = len(str(end))
    return "\n".join(f"{idx:>{width}}: {lines[idx - 1]}" for idx in range(start, end + 1))


def _nearby_symbol_hints(path: Path, line: int) -> str:
    try:
        lines = path.read_text(errors="replace").splitlines()
    except Exception:
        return ""
    patterns = [
        re.compile(r"^\s*(class|def|async def)\s+([\w_]+)"),
        re.compile(r"^\s*(function)\s+([\w_$]+)"),
        re.compile(r"^\s*(const|let|var)\s+([\w_$]+)\s*=\s*(async\s*)?(\([^)]*\)|[\w_$]+)\s*=>"),
        re.compile(r"^\s*(export\s+)?(class|function)\s+([\w_$]+)"),
        re.compile(r"^\s*(import\s+.+|from\s+\S+\s+import\s+.+)"),
    ]
    hits: list[str] = []
    for idx, raw in enumerate(lines, start=1):
        if abs(idx - line) > 160 and not raw.lstrip().startswith(("import ", "from ")):
            continue
        for pattern in patterns:
            if pattern.search(raw):
                hits.append(f"{idx}: {raw.strip()}")
                break
    return "\n".join(hits[:40])


def _safe_int(value, default: int) -> int:
    try:
        return int(value)
    except Exception:
        return default


def _fence(text: str, language: str = "") -> str:
    language = re.sub(r"[^\w+-]", "", language or "")
    return f"```{language}\n{text}\n```"
