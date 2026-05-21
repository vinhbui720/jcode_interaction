from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import json
import re

CONTEXT_DIR = Path.home() / ".local" / "state" / "jcode-panel" / "contexts"
VSCODE_CONTEXT_PATH = CONTEXT_DIR / "vscode.json"
OBSIDIAN_CONTEXT_PATH = CONTEXT_DIR / "obsidian.json"

INTERACTION_SOURCES = {"vscode", "obsidian"}
CHIP_LABELS = {
    "vscode": "vscode",
    "obsidian": "obsidian",
}
# @vscode, @obsidian anywhere in the prompt. Slash remains reserved for jcode commands.
INTERACTION_TAG_RE = re.compile(r"(?<![\w\[])(@)(vscode|obsidian)\b", re.IGNORECASE)
# Accept new visual chips and legacy chips for backward compatibility.
INTERACTION_CHIP_RE = re.compile(r"(?:\{\{\s*[@/]\s*(vscode|obsidian)\s*\}\}|⟦\s*[@/]\s*(vscode|obsidian)\s*⟧|\[(vscode|obsidian)\])", re.IGNORECASE)
INTERACTION_CHIP_DELETE_RE = re.compile(r"(?:\{\{\s*[@/]\s*(?:vscode|obsidian)\s*\}\}|⟦\s*[@/]\s*(?:vscode|obsidian)\s*⟧|\[(?:vscode|obsidian)\])\s*$", re.IGNORECASE)
INTERACTION_PARTIAL_RE = re.compile(r"(?<![\w\[])(@)([a-zA-Z_][\w-]*)?$")


@dataclass
class InteractionContext:
    source: str
    title: str
    body: str


class InteractionContextError(RuntimeError):
    pass


def chip_for_source(source: str, marker: str = "@") -> str:
    source = source.lower().strip()
    return f"{{{{@{CHIP_LABELS.get(source, source)}}}}}"


def normalize_interaction_tags(text: str) -> str:
    """Turn @source or /source mentions into lightweight editable chips."""
    return INTERACTION_TAG_RE.sub(lambda m: chip_for_source(m.group(2), m.group(1)), text)


def normalize_interaction_tags_with_cursor(text: str, cursor: int | None = None) -> tuple[str, int]:
    """Normalize complete interaction tags while preserving cursor position.

    Gtk.Entry sometimes reports cursor -1 during changed events. Treat that as
    end-of-text and compute the new cursor by summing replacements before it,
    rather than using a whole-string length delta that can place the cursor in
    the middle of a chip.
    """
    if cursor is None or cursor < 0:
        cursor = len(text)
    output: list[str] = []
    new_cursor = cursor
    last = 0
    for match in INTERACTION_TAG_RE.finditer(text):
        chip = chip_for_source(match.group(2), match.group(1))
        output.append(text[last:match.start()])
        output.append(chip)
        if match.end() <= cursor:
            new_cursor += len(chip) - (match.end() - match.start())
        last = match.end()
    output.append(text[last:])
    normalized = "".join(output)
    return normalized, max(0, min(len(normalized), new_cursor))


def complete_interaction_token(text: str, cursor: int | None = None) -> tuple[str, int, bool]:
    """Complete/convert the @ or / token immediately before cursor.

    Returns (new_text, new_cursor, changed). This is used by Tab/Space/Enter so
    interaction commands work even when the user expects command completion
    rather than typing the full token and waiting for the changed signal.
    """
    if cursor is None or cursor < 0:
        cursor = len(text)
    prefix = text[:cursor]
    match = INTERACTION_PARTIAL_RE.search(prefix)
    if not match:
        return text, cursor, False
    raw = (match.group(2) or "").lower()
    matches = [source for source in sorted(INTERACTION_SOURCES) if source.startswith(raw)]
    if len(matches) != 1:
        return text, cursor, False
    chip = chip_for_source(matches[0], match.group(1))
    new_text = text[:match.start()] + chip + text[cursor:]
    return new_text, match.start() + len(chip), True


def interaction_token_hints(text: str, cursor: int | None = None) -> list[str]:
    if cursor is None or cursor < 0:
        cursor = len(text)
    match = INTERACTION_PARTIAL_RE.search(text[:cursor])
    if not match:
        return []
    marker = match.group(1)
    raw = (match.group(2) or "").lower()
    return [marker + source for source in sorted(INTERACTION_SOURCES) if source.startswith(raw)]


def interaction_sources(text: str) -> list[str]:
    sources: list[str] = []
    for match in INTERACTION_CHIP_RE.finditer(text):
        sources.append(next(group for group in match.groups() if group).lower())
    return sources


def strip_interaction_chips(text: str) -> str:
    return re.sub(r"[ \t]{2,}", " ", INTERACTION_CHIP_RE.sub("", text)).strip()


def expand_interaction_chips(text: str) -> str:
    """Expand every interaction chip occurrence into an on-demand context block."""
    sources = interaction_sources(text)
    if not sources:
        return text
    prompt_text = strip_interaction_chips(text)
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
