from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re
from urllib.parse import unquote, urlparse

MAX_SELECTED_TEXT_CHARS = 4000
URL_RE = re.compile(r"https?://[^\s<>'\")]+", re.IGNORECASE)
POPUP_CONTEXT_CHIP_DELETE_RE = re.compile(r"(?:\[(?:text:[^\]]+|link:[^\]]+|file:[^\]]+|\d+ files)\])\s*$", re.IGNORECASE)
POPUP_CONTEXT_CHIP_RE = re.compile(r"\[(?:text:[^\]]+|link:[^\]]+|file:[^\]]+|\d+ files)\]", re.IGNORECASE)


@dataclass
class PopupContextChip:
    tag: str
    body: str
    kind: str


def build_popup_context_chips(
    *,
    selected_text: str = "",
    file_paths: list[str] | None = None,
    app: str = "",
    window_title: str = "",
    file_path: str = "",
    line: int | None = None,
) -> list[PopupContextChip]:
    """Build editable prompt chips from the context available when popup opens.

    This intentionally returns plain ASCII tags because Gtk.Entry cursor handling is
    reliable with those, unlike embedded widgets or unicode-heavy visual chips.
    """
    chips: list[PopupContextChip] = []
    seen: set[str] = set()

    selection = _clean(selected_text)
    selection_url = _first_url(selection)
    if selection:
        if selection_url:
            _append_unique(chips, seen, _link_chip(selection_url))
        else:
            chips.append(_text_chip(selection, app=app, window_title=window_title, file_path=file_path, line=line))

    files = [str(Path(p).expanduser()) for p in (file_paths or []) if p]
    if files:
        chips.append(_files_chip(files))

    return chips


def expand_popup_context_chips(text: str, chips: list[PopupContextChip]) -> str:
    if not chips:
        return text
    remaining = text
    blocks: list[str] = []
    for chip in chips:
        if chip.tag in remaining:
            remaining = remaining.replace(chip.tag, " ")
            blocks.append(chip.body)
    prompt = re.sub(r"[ \t]{2,}", " ", remaining).strip()
    if not blocks:
        return text
    tail = "User prompt:\n" + prompt if prompt else "User prompt:\n"
    return "\n\n".join([*blocks, tail]).strip()


def strip_popup_context_chips(text: str) -> str:
    return re.sub(r"[ \t]{2,}", " ", POPUP_CONTEXT_CHIP_RE.sub("", text)).strip()


def _append_unique(chips: list[PopupContextChip], seen: set[str], chip: PopupContextChip) -> None:
    key = chip.body
    if key not in seen:
        chips.append(chip)
        seen.add(key)


def _text_chip(text: str, *, app: str, window_title: str, file_path: str, line: int | None) -> PopupContextChip:
    label = _text_label(file_path, window_title, line)
    limited, truncated = _limit_text(text, MAX_SELECTED_TEXT_CHARS)
    parts = ["Context: selected text"]
    if app:
        parts.append(f"app: {app}")
    if window_title:
        parts.append(f"window: {window_title}")
    if file_path:
        parts.append(f"file: {file_path}")
    if line:
        parts.append(f"line: {line}")
    if truncated:
        parts.append(f"note: selected text was truncated to {MAX_SELECTED_TEXT_CHARS} characters")
    parts.extend(["selected text:", limited])
    return PopupContextChip(tag=f"[text:{label}]", body="\n".join(parts), kind="text")


def _link_chip(url: str) -> PopupContextChip:
    parsed = urlparse(url)
    host = parsed.netloc or parsed.path.split("/")[0] or "link"
    label = _safe_label(host, fallback="link", max_len=36)
    return PopupContextChip(tag=f"[link:{label}]", body=f"Context: link\nurl: {url}", kind="link")


def _files_chip(paths: list[str], current_file: bool = False) -> PopupContextChip:
    clean_paths = [str(Path(p).expanduser()) for p in paths if p]
    if len(clean_paths) == 1:
        name = Path(clean_paths[0]).name or clean_paths[0]
        tag = f"[file:{_safe_label(name, fallback='file', max_len=42)}]"
        title = "Context: current file" if current_file else "Context: selected file"
        body = f"{title}\npath: {clean_paths[0]}"
        return PopupContextChip(tag=tag, body=body, kind="file")
    tag = f"[{len(clean_paths)} files]"
    body = "Context: selected files\npaths:\n" + "\n".join(f"- {p}" for p in clean_paths)
    return PopupContextChip(tag=tag, body=body, kind="file")


def _text_label(file_path: str, window_title: str, line: int | None) -> str:
    if file_path:
        base = Path(file_path).name or "selection"
        suffix = f":{line}" if line else ""
        return _safe_label(base + suffix, fallback="selection", max_len=42)
    if window_title:
        return _safe_label(window_title, fallback="selection", max_len=42)
    return "selection"


def _file_paths_from_uris(raw: str) -> list[str]:
    paths: list[str] = []
    for token in re.split(r"[\r\n]+", raw or ""):
        token = token.strip()
        if not token:
            continue
        if token.startswith("file://"):
            parsed = urlparse(token)
            paths.append(unquote(parsed.path))
        elif token.startswith("/") and Path(token).exists():
            paths.append(token)
    return list(dict.fromkeys(paths))[:50]


def _first_url(text: str) -> str:
    match = URL_RE.search(text or "")
    return match.group(0).rstrip(".,;:]") if match else ""


def _limit_text(text: str, limit: int) -> tuple[str, bool]:
    text = _clean(text)
    if len(text) <= limit:
        return text, False
    head = max(1, limit // 2)
    tail = max(1, limit - head - 80)
    return text[:head].rstrip() + "\n... [truncated] ...\n" + text[-tail:].lstrip(), True


def _clean(text: str) -> str:
    return (text or "").replace("\r\n", "\n").replace("\r", "\n").strip()


def _safe_label(text: str, *, fallback: str, max_len: int) -> str:
    label = re.sub(r"[\[\]\n\r\t]+", " ", text or "").strip()
    label = re.sub(r"\s+", " ", label)
    if len(label) > max_len:
        label = label[: max_len - 1].rstrip() + "…"
    return label or fallback
