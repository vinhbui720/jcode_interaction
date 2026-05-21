from __future__ import annotations

import os
# Force XWayland/X11 on GNOME Wayland so floating prompt positioning works.
# Native Wayland intentionally prevents arbitrary window placement.
os.environ.setdefault("GDK_BACKEND", "x11")

import html
import re
import threading
import time
import subprocess
import json
import tempfile
import shutil
from urllib.parse import unquote, urlparse
from dataclasses import dataclass
from pathlib import Path
from hashlib import sha256

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("AppIndicator3", "0.1")
from gi.repository import AppIndicator3, GLib, Gtk, Gdk, Gio  # type: ignore

from .config import AppConfig, CONFIG_PATH
from .control import ControlResponse, ControlServer
from .services import AppController
from .context import BrowserBridge, capture_active_context
from .dropdown import ConversationBuffer
from .floating import CompletionState
from .diagnostics import append_log
from .jcode_client import JcodeClient, JcodeUnavailable
from .protocol import PanelEvent, PanelEventKind, activity_is_terminal, activity_label, activity_state
from .hotkeys import MODIFIERS, hotkey_parts, key_name_from_pynput, normalize_hotkey, normalize_key_name
from .notify import notify
from .terminal import launch
from .style import add_class, load_css
from .positioning import xdotool_mouse_position_full
from .updater import self_update
from .interaction_context import InteractionContextError, INTERACTION_CHIP_DELETE_RE, complete_interaction_token, expand_interaction_chips, interaction_token_hints


def markdown_to_pango(text: str) -> str:
    """Render a small, safe markdown subset as Pango markup for GTK labels."""
    lines: list[str] = []
    in_fence = False
    for raw_line in (text or "").strip().splitlines():
        line = raw_line.rstrip()
        if line.strip().startswith("```"):
            in_fence = not in_fence
            continue
        escaped = html.escape(line)
        if in_fence:
            lines.append(f'<span foreground="#0f766e" font_family="monospace">{escaped}</span>')
            continue
        stripped = line.strip()
        if stripped.startswith("#"):
            title = html.escape(stripped.lstrip("#").strip())
            lines.append(f'<span foreground="#7c3aed" weight="bold" size="larger">{title}</span>')
            continue
        bullet = re.match(r"^(\s*)([-*•]|\d+\.)\s+(.*)$", line)
        if bullet:
            body = _inline_markdown_to_pango(bullet.group(3))
            lines.append(f'<span foreground="#06b6d4">●</span> {body}')
            continue
        if not stripped:
            lines.append("")
            continue
        lines.append(_inline_markdown_to_pango(line))
    return "\n".join(lines)


def _inline_markdown_to_pango(text: str) -> str:
    escaped = html.escape(text)
    escaped = re.sub(r"`([^`]+)`", r'<span foreground="#0f766e" font_family="monospace">\1</span>', escaped)
    escaped = re.sub(r"\*\*([^*]+)\*\*", r'<span foreground="#2563eb" weight="bold">\1</span>', escaped)
    escaped = re.sub(r"\*([^*]+)\*", r'<span foreground="#9333ea" style="italic">\1</span>', escaped)
    return escaped


def format_stream_lines(text: str, max_lines: int = 9) -> str:
    """Keep streamed assistant text readable by showing recent logical lines."""
    normalized = (text or "").replace("\r\n", "\n").replace("\r", "\n")
    lines = normalized.split("\n")
    if len(lines) == 1 and len(lines[0]) > 360:
        # Long no-newline streams are hard to read in a toast. Soft-wrap them
        # into line-sized chunks without changing the conversation buffer.
        chunks = re.findall(r".{1,120}(?:\s+|$)", lines[0]) or [lines[0]]
        lines = [chunk.strip() for chunk in chunks if chunk.strip()]
    return "\n".join(lines[-max_lines:])


def token_notice_from_raw(raw: dict | None) -> str:
    if not raw:
        return ""
    candidates = [raw]
    for key in ("usage", "tokens", "token_usage", "metrics"):
        value = raw.get(key)
        if isinstance(value, dict):
            candidates.append(value)
    for data in candidates:
        parts = []
        for key, label in (("input_tokens", "in"), ("prompt_tokens", "in"), ("output_tokens", "out"), ("completion_tokens", "out"), ("total_tokens", "total")):
            value = data.get(key)
            if isinstance(value, (int, float)):
                parts.append(f"{label} {int(value)}")
        if parts:
            return "tokens: " + ", ".join(dict.fromkeys(parts))
    return ""


def parse_screenshot_command(arg: str) -> tuple[str, str]:
    """Return (mode, prompt) for /screenshot args.

    Supported modes: ask, area, full. Default is ask so users can choose.
    """
    arg = (arg or "").strip()
    if not arg:
        return "ask", "Analyze this screenshot."
    parts = arg.split(maxsplit=1)
    first = parts[0].lower()
    aliases = {
        "area": "area",
        "region": "area",
        "select": "area",
        "drag": "area",
        "full": "full",
        "screen": "full",
        "whole": "full",
        "all": "full",
    }
    if first in aliases:
        prompt = parts[1].strip() if len(parts) > 1 else "Analyze this screenshot."
        return aliases[first], prompt
    return "ask", arg


SCREENSHOT_TAG_RE = re.compile(r"\[(?:screenshot:[^\]]+|pic\d+)\]\s*$")
PIC_TAG_RE = re.compile(r"\[pic(\d+)\]")


def screenshot_tag(path: str) -> str:
    return f"[screenshot:{path}]"


def pic_tag(index: int) -> str:
    return f"[pic{index}]"


@dataclass
class LiveActivity:
    label: str = "idle"
    state: str = "idle"
    started_at: float = 0.0
    active: bool = False

    def elapsed(self, now: float | None = None) -> int:
        if not self.active or not self.started_at:
            return 0
        return max(0, int((now or time.monotonic()) - self.started_at))


class FloatingInput(Gtk.Window):
    def __init__(self, app: "PanelApp"):
        super().__init__(type=Gtk.WindowType.TOPLEVEL)
        self.app = app
        self.context_enabled = True
        self.completions = CompletionState()
        self.set_decorated(False)
        self.set_keep_above(True)
        self.set_skip_taskbar_hint(True)
        self.set_skip_pager_hint(True)
        # While the popup is open, keyboard input must belong to the panel, not
        # the app underneath. Accept focus and then explicitly grab the keyboard.
        self.set_accept_focus(True)
        self.set_focus_on_map(True)
        self.set_type_hint(Gdk.WindowTypeHint.UTILITY)
        self.follow_mouse = False
        self.follow_source_id = 0
        self.current_x: float | None = None
        self.current_y: float | None = None
        self.target_x: float | None = None
        self.target_y: float | None = None
        self.typed_once = False
        self.set_border_width(0)
        self.set_opacity(1.0)
        self.set_app_paintable(True)
        screen = self.get_screen()
        visual = screen.get_rgba_visual() if screen else None
        if visual:
            self.set_visual(visual)
        self.connect("draw", self._draw_transparent)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        add_class(box, "floating-root")
        self.entry = Gtk.Entry()
        self.entry.set_can_focus(True)
        self.entry.connect("activate", self._on_enter)
        self.entry.connect("key-press-event", self._on_key)
        self.entry.connect("changed", self._on_changed)
        self.connect("key-press-event", self._on_key)
        self.connect("button-press-event", self._on_pointer_interaction)
        self.connect("focus-out-event", self._on_focus_out)
        box.pack_start(self.entry, True, True, 0)
        self.slash_hint = Gtk.Label(label="")
        self.slash_hint.set_xalign(0)
        self.slash_hint.set_line_wrap(True)
        add_class(self.slash_hint, "slash-hint")
        box.pack_start(self.slash_hint, False, False, 0)
        self.add(box)
        self._resize_for_suggestions(False)
        self.target_window_id = ""
        self.keyboard_grabbed = False
        self.suppress_listener = None
        self.submitting = False

    def _draw_transparent(self, _widget, cr):
        cr.set_source_rgba(0, 0, 0, 0)
        cr.set_operator(0)  # cairo.OPERATOR_CLEAR without importing cairo
        cr.paint()
        cr.set_operator(2)  # cairo.OPERATOR_OVER
        return False

    def show_at_pointer(self, initial_text: str = ""):
        x, y, window_id = xdotool_mouse_position_full()
        self.target_window_id = window_id
        self.context_enabled = self.app.config.session.send_context_default
        self.entry.set_text(initial_text)
        if initial_text:
            self.entry.set_position(-1)
        self.entry.set_placeholder_text("Ask jcode...")
        self._update_slash_hint("")
        self.typed_once = False
        self.submitting = False
        self.follow_mouse = True
        self.current_x = None
        self.current_y = None

        self.show_all()
        self.realize()
        self._start_suppress_keyboard_listener()
        self._follow_mouse_tick(initial=(x, y))
        if self.follow_source_id:
            GLib.source_remove(self.follow_source_id)
        self.follow_source_id = GLib.timeout_add(16, self._follow_mouse_tick)
        self.present_with_time(Gtk.get_current_event_time())
        self.set_focus(self.entry)
        self.entry.grab_focus()
        GLib.idle_add(self._focus_entry)
        GLib.timeout_add(25, self._focus_entry)
        GLib.timeout_add(75, self._focus_entry)
        GLib.timeout_add(160, self._focus_entry)
        GLib.timeout_add(40, self._grab_keyboard)
        GLib.timeout_add(120, self._grab_keyboard)
        GLib.timeout_add(260, self._grab_keyboard)

    def _follow_mouse_tick(self, initial: tuple[int | None, int | None] | None = None) -> bool:
        if not self.follow_mouse or not self.get_visible():
            self.follow_source_id = 0
            return False
        x, y = initial if initial is not None else self._fast_mouse_position()
        if x is None or y is None:
            return True
        self.target_x = float(max(0, x + 20))
        self.target_y = float(max(0, y + 24))
        if self.current_x is None or self.current_y is None:
            self.current_x, self.current_y = self.target_x, self.target_y
        else:
            # Smooth but responsive realtime tracking.
            alpha = 0.55
            self.current_x += (self.target_x - self.current_x) * alpha
            self.current_y += (self.target_y - self.current_y) * alpha
            if abs(self.target_x - self.current_x) < 1:
                self.current_x = self.target_x
            if abs(self.target_y - self.current_y) < 1:
                self.current_y = self.target_y
        self.move(int(self.current_x), int(self.current_y))
        return True

    def _on_pointer_interaction(self, *_args):
        return False

    def _on_changed(self, _entry):
        old_text = self.entry.get_text()
        text = old_text.strip()
        if text:
            self.typed_once = True
        self.completions.update([])
        self._update_slash_hint(self.entry.get_text().strip())

    def _update_slash_hint(self, text: str) -> None:
        if not hasattr(self, "slash_hint"):
            return
        interaction_hints = interaction_token_hints(self.entry.get_text(), self.entry.get_position())
        known = ["/model", "/usage", "/ustage", "/screen-shot", "/help", "/resume", "/clear", "/compact", "/skill", "/memory"]
        slash_matches = [item for item in known if text.startswith("/") and item.startswith(text)] if text.startswith("/") else []
        if interaction_hints or slash_matches:
            parts = []
            if interaction_hints:
                parts.append("Chip: " + "   ".join(interaction_hints[:4]))
            if slash_matches:
                parts.append("Cmd: " + "   ".join(slash_matches[:6]))
            self.slash_hint.set_text("Tab/Space/Enter · " + "  |  ".join(parts))
            self.slash_hint.show()
            self._resize_for_suggestions(True)
            return
        if not text.startswith("/"):
            self.slash_hint.set_text("")
            self.slash_hint.hide()
            self._resize_for_suggestions(False)
            return
        matches = [item for item in known if item.startswith(text)] or known[:6]
        self.slash_hint.set_text("Tab complete · Enter run · " + "   ".join(matches[:6]))
        self.slash_hint.show()
        self._resize_for_suggestions(True)

    def _resize_for_suggestions(self, has_suggestions: bool) -> None:
        width = max(320, min(1200, int(getattr(self.app.config.ui, "popup_width", 520) or 520)))
        slim_height = max(32, min(220, int(getattr(self.app.config.ui, "popup_height", 42) or 42)))
        height = slim_height + (38 if has_suggestions else 0)
        self.set_default_size(width, height)
        if self.get_visible():
            self.resize(width, height)

    def _activate_self(self) -> None:
        try:
            window = self.get_window()
            xid = window.get_xid() if window and hasattr(window, "get_xid") else None
            if xid:
                subprocess.Popen(["xdotool", "windowactivate", "--sync", str(xid)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except Exception:
            pass

    def _grab_keyboard(self) -> bool:
        if not self.get_visible():
            return False
        try:
            window = self.get_window()
            display = Gdk.Display.get_default()
            seat = display.get_default_seat() if display else None
            if window and seat:
                result = seat.grab(
                    window,
                    Gdk.SeatCapabilities.KEYBOARD,
                    False,  # owner_events=false blocks keyboard delivery to other apps
                    None,
                    None,
                    None,
                )
                self.keyboard_grabbed = result == Gdk.GrabStatus.SUCCESS
                if not self.keyboard_grabbed:
                    append_log(f"Keyboard grab did not succeed: {result}")
        except Exception as exc:
            append_log(f"Keyboard grab failed: {exc}")
        self._focus_entry()
        return False

    def _release_keyboard(self) -> None:
        try:
            display = Gdk.Display.get_default()
            seat = display.get_default_seat() if display else None
            if seat and self.keyboard_grabbed:
                seat.ungrab()
        except Exception:
            pass
        self._stop_suppress_keyboard_listener()
        self.keyboard_grabbed = False
        self.suppress_listener = None

    def _start_suppress_keyboard_listener(self) -> None:
        if self.suppress_listener:
            return
        try:
            from pynput import keyboard  # type: ignore

            def on_press(key):
                GLib.idle_add(self.app._route_ambient_key, key, True, True)

            def on_release(key):
                GLib.idle_add(self.app._route_ambient_key, key, False, True)

            self.suppress_listener = keyboard.Listener(on_press=on_press, on_release=on_release, suppress=True)
            self.suppress_listener.daemon = True
            self.suppress_listener.start()
        except Exception as exc:
            append_log(f"Suppressing keyboard listener unavailable: {exc}")

    def _stop_suppress_keyboard_listener(self) -> None:
        listener = self.suppress_listener
        self.suppress_listener = None
        if listener:
            try:
                listener.stop()
            except Exception:
                pass

    def _on_focus_out(self, *_args):
        # Keep keyboard focus biased to the input, but do not capture mouse.
        if self.get_visible():
            GLib.timeout_add(20, self._focus_entry)
        return False

    def _focus_entry(self) -> bool:
        if self.get_visible():
            self.present_with_time(Gtk.get_current_event_time())
            self._activate_self()
            self.set_focus(self.entry)
            self.entry.grab_focus()
        return False

    def hide(self):
        self.follow_mouse = False
        self.typed_once = False
        self._release_keyboard()
        if self.follow_source_id:
            GLib.source_remove(self.follow_source_id)
            self.follow_source_id = 0
        super().hide()

    def _fast_mouse_position(self) -> tuple[int | None, int | None]:
        x, y, _window_id = xdotool_mouse_position_full()
        if x is not None and y is not None:
            return x, y
        display = Gdk.Display.get_default()
        seat = display.get_default_seat() if display else None
        pointer = seat.get_pointer() if seat else None
        if pointer:
            _screen, x, y = pointer.get_position()
            return x, y
        return None, None


    def append_text(self, text: str) -> None:
        if not text:
            return
        current = self.entry.get_text()
        pos = self.entry.get_position()
        if pos < 0:
            pos = len(current)
        updated = current[:pos] + text + current[pos:]
        self.entry.set_text(updated)
        self.entry.set_position(pos + len(text))

    def backspace(self) -> None:
        current = self.entry.get_text()
        pos = self.entry.get_position()
        if pos < 0:
            pos = len(current)
        prefix = current[:pos]
        match = SCREENSHOT_TAG_RE.search(prefix)
        if match:
            self.entry.set_text(current[:match.start()] + current[pos:])
            self.entry.set_position(match.start())
            return
        tag_match = INTERACTION_CHIP_DELETE_RE.search(prefix)
        if tag_match:
            self.entry.set_text(current[:tag_match.start()] + current[pos:])
            self.entry.set_position(tag_match.start())
            return
        if pos > 0:
            self.entry.set_text(current[:pos - 1] + current[pos:])
            self.entry.set_position(pos - 1)

    def submit(self) -> None:
        self._on_enter(self.entry)

    def _on_enter(self, _entry):
        if self.submitting:
            return
        if self._complete_interaction_at_cursor():
            return
        self.submitting = True
        text = self.entry.get_text().strip()
        if text.startswith("/") and self.app.handle_slash_command(text):
            self.hide()
            return
        self.app.active_context = self._capture_context_on_submit()
        self.hide()
        if text:
            self.app.send_prompt(text, self.context_enabled)

    def _capture_context_on_submit(self):
        _x, _y, window_id = xdotool_mouse_position_full()
        ctx = capture_active_context(window_id or self.target_window_id)
        primary = self._clipboard_text(Gdk.SELECTION_PRIMARY)
        clipboard = self._clipboard_text(Gdk.SELECTION_CLIPBOARD)
        uris = self._clipboard_uris(Gdk.SELECTION_CLIPBOARD)
        ctx.selected_text = primary
        ctx.clipboard_text = "\n".join([x for x in [clipboard, uris] if x])
        return ctx

    def _clipboard_text(self, selection) -> str:
        try:
            text = Gtk.Clipboard.get(selection).wait_for_text()
            return (text or "").strip()[:4000]
        except Exception:
            return ""

    def _clipboard_uris(self, selection) -> str:
        try:
            uris = Gtk.Clipboard.get(selection).wait_for_uris() or []
            return "\n".join(str(u) for u in uris)[:4000]
        except Exception:
            return ""

    def _on_key(self, _widget, event):
        key = Gdk.keyval_name(event.keyval)
        alt = bool(event.state & Gdk.ModifierType.MOD1_MASK)
        if key == "Escape":
            self.app.cancel_input_mode()
            return True
        if alt and key and key.lower() == "c":
            self.context_enabled = not self.context_enabled
            return True
        if key in {"space", "KP_Space"} and self._complete_interaction_at_cursor(add_trailing_space=True):
            return True
        if key in {"Tab", "ISO_Left_Tab", "KP_Tab"}:
            if self._complete_interaction_at_cursor(add_trailing_space=True):
                return True
            text = self.entry.get_text()
            if text.startswith("/"):
                if not self.completions.items:
                    self.completions.update(self.app.client.completions(text))
                suggestion = self.completions.tab()
                if suggestion:
                    self.entry.set_text(suggestion)
                    self.entry.set_position(-1)
                    self._update_slash_hint(suggestion)
            return True
        return False

    def _complete_interaction_at_cursor(self, add_trailing_space: bool = False) -> bool:
        text = self.entry.get_text()
        updated, pos, changed = complete_interaction_token(text, self.entry.get_position())
        if not changed:
            return False
        if add_trailing_space and (pos >= len(updated) or updated[pos:pos + 1] != " "):
            updated = updated[:pos] + " " + updated[pos:]
            pos += 1
        self.entry.set_text(updated)
        self.entry.set_position(-1)
        self._update_slash_hint(updated.strip())
        return True


class Dropdown(Gtk.Window):
    def __init__(self, app: "PanelApp"):
        super().__init__(type=Gtk.WindowType.TOPLEVEL)
        self.app = app
        self.set_title("jcode-panel")
        self.set_default_size(460, 420)
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        add_class(root, "panel-root")
        self.text = Gtk.TextView(editable=False, cursor_visible=False, monospace=True)
        self.text.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.buffer = self.text.get_buffer()
        scroller = Gtk.ScrolledWindow()
        scroller.add(self.text)
        root.pack_start(scroller, True, True, 0)
        actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        for label, cb in [
            ("⌨ Same session", self.app.open_terminal),
            ("▣ Panel", self.app.show_prompt),
            ("+ New", self.app.new_session),
            ("↩ Resume", self.app.resume_session),
            ("⚙ Settings", self.app.show_settings),
        ]:
            btn = Gtk.Button(label=label)
            btn.connect("clicked", lambda _b, c=cb: c())
            actions.pack_start(btn, True, True, 0)
        root.pack_start(actions, False, False, 0)
        self.add(root)
        self.connect("delete-event", self._hide)

    def _hide(self, *_args):
        self.hide()
        return True

    def refresh(self):
        session = self.app.controller.active_session or "new session pending first prompt"
        header = f"section: {self.app.controller.active_session_name}\nsession: {session}"
        lines = [header] + [f"{who}: {text}" for who, text in self.app.conversation.messages]
        self.buffer.set_text("\n\n".join(lines))


class AnswerToast(Gtk.Window):
    def __init__(self, app: "PanelApp"):
        super().__init__(type=Gtk.WindowType.TOPLEVEL)
        self.app = app
        self.set_decorated(False)
        self.set_keep_above(True)
        self.set_skip_taskbar_hint(True)
        self.set_skip_pager_hint(True)
        self.set_accept_focus(False)
        self.set_type_hint(Gdk.WindowTypeHint.NOTIFICATION)
        self.set_opacity(1.0)
        self.set_app_paintable(True)
        screen = self.get_screen()
        visual = screen.get_rgba_visual() if screen else None
        if visual:
            self.set_visual(visual)
        self.connect("draw", self._draw_transparent)
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        add_class(root, "toast-root")
        title = Gtk.Label(label="jcode feedback")
        title.set_xalign(0)
        add_class(title, "toast-title")
        self.label = Gtk.Label(label="")
        self.label.set_use_markup(True)
        self.label.set_xalign(0)
        self.label.set_yalign(0)
        self.label.set_line_wrap(True)
        self.label.set_selectable(True)
        self.label.set_max_width_chars(70)
        add_class(self.label, "toast-text")
        self.notice = Gtk.Label(label="")
        self.notice.set_use_markup(True)
        self.notice.set_xalign(0)
        self.notice.set_line_wrap(True)
        add_class(self.notice, "toast-notice")
        actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        open_btn = self._icon_button("view-list-symbolic", "Open conversation")
        open_btn.connect("clicked", lambda _b: self.app.show_dropdown())
        prompt_btn = self._icon_button("mail-reply-sender-symbolic", "Reply")
        prompt_btn.connect("clicked", lambda _b: self.app.show_prompt())
        close_btn = self._icon_button("window-close-symbolic", "Dismiss")
        close_btn.connect("clicked", lambda _b: self.app.stop_answering("dismissed"))
        actions.pack_start(open_btn, False, False, 0)
        actions.pack_start(prompt_btn, False, False, 0)
        actions.pack_end(close_btn, False, False, 0)
        root.pack_start(title, False, False, 0)
        root.pack_start(self.label, True, True, 0)
        root.pack_start(self.notice, False, False, 0)
        root.pack_start(actions, False, False, 0)
        self.add(root)
        self.set_default_size(520, 220)
        self.hide_source_id = 0
        self.refresh_source_id = 0
        self.pending_feedback = ""
        self.pending_notice = ""

    def _draw_transparent(self, _widget, cr):
        cr.set_source_rgba(0, 0, 0, 0)
        cr.set_operator(0)  # cairo.OPERATOR_CLEAR without importing cairo
        cr.paint()
        cr.set_operator(2)  # cairo.OPERATOR_OVER
        return False

    def _icon_button(self, icon_name: str, tooltip: str) -> Gtk.Button:
        image = Gtk.Image.new_from_icon_name(icon_name, Gtk.IconSize.BUTTON)
        button = Gtk.Button()
        button.set_image(image)
        button.set_always_show_image(True)
        button.set_tooltip_text(tooltip)
        add_class(button, "toast-icon-button")
        return button

    def update_feedback(self, text: str, notice: str = ""):
        text = format_stream_lines(text.strip())
        if not text:
            return
        self.pending_feedback = text[-1600:]
        if notice:
            self.pending_notice = notice
        if not self.refresh_source_id:
            self.refresh_source_id = GLib.timeout_add(80, self._flush_feedback)
        self.show_all()
        self._move_to_corner()
        self._reset_idle_hide_timer()

    def update_notice(self, notice: str):
        if not notice.strip():
            return
        self.pending_notice = notice.strip()
        if not self.refresh_source_id:
            self.refresh_source_id = GLib.timeout_add(80, self._flush_feedback)
        self.show_all()
        self._move_to_corner()
        self._reset_idle_hide_timer()

    def _flush_feedback(self) -> bool:
        self.refresh_source_id = 0
        self.label.set_markup(markdown_to_pango(self.pending_feedback))
        if self.pending_notice:
            self.notice.set_markup(f'<span foreground="#64748b">{html.escape(self.pending_notice)}</span>')
            self.notice.show()
        else:
            self.notice.hide()
        self.show_all()
        self._move_to_corner()
        return False

    def hide(self):
        if self.hide_source_id:
            GLib.source_remove(self.hide_source_id)
            self.hide_source_id = 0
        if self.refresh_source_id:
            GLib.source_remove(self.refresh_source_id)
            self.refresh_source_id = 0
        super().hide()

    def dismiss(self):
        self.hide()

    def _reset_idle_hide_timer(self):
        if self.hide_source_id:
            GLib.source_remove(self.hide_source_id)
        self.hide_source_id = GLib.timeout_add_seconds(60, self._idle_hide)

    def _idle_hide(self) -> bool:
        self.hide_source_id = 0
        self.hide()
        return False

    def _move_to_corner(self):
        screen = Gdk.Screen.get_default()
        if not screen:
            return
        monitor = screen.get_monitor_at_point(0, 0)
        geo = screen.get_monitor_geometry(monitor)
        width, height = self.get_size()
        self.move(max(0, geo.x + geo.width - width - 24), max(0, geo.y + geo.height - height - 48))


class HotkeyCaptureDialog(Gtk.Dialog):
    def __init__(self, parent: Gtk.Window, current: str):
        super().__init__(title="Record hotkey", transient_for=parent, flags=0)
        self.set_modal(True)
        self.set_default_size(380, 130)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("Use hotkey", Gtk.ResponseType.OK)
        self.captured = normalize_hotkey(current)
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, margin=14)
        add_class(root, "modern-card")
        title = Gtk.Label(label="Press the new hotkey combo")
        title.set_xalign(0)
        add_class(title, "modern-title")
        self.preview = Gtk.Label(label=self.captured)
        self.preview.set_xalign(0)
        add_class(self.preview, "hotkey-preview")
        hint = Gtk.Label(label="Examples: F8, Ctrl+Alt+J, Shift+Super+Space. Press Esc to cancel.")
        hint.set_xalign(0)
        hint.set_line_wrap(True)
        add_class(hint, "modern-subtitle")
        root.pack_start(title, False, False, 0)
        root.pack_start(self.preview, False, False, 0)
        root.pack_start(hint, False, False, 0)
        self.get_content_area().add(root)
        self.connect("key-press-event", self._on_key_press)
        self.show_all()

    def _on_key_press(self, _widget, event):
        key = normalize_key_name(Gdk.keyval_name(event.keyval) or "")
        if key == "escape":
            self.response(Gtk.ResponseType.CANCEL)
            return True
        if key in MODIFIERS:
            return True
        mods: list[str] = []
        if event.state & Gdk.ModifierType.CONTROL_MASK:
            mods.append("ctrl")
        if event.state & Gdk.ModifierType.MOD1_MASK:
            mods.append("alt")
        if event.state & Gdk.ModifierType.SHIFT_MASK:
            mods.append("shift")
        if event.state & Gdk.ModifierType.SUPER_MASK:
            mods.append("super")
        self.captured = normalize_hotkey("+".join(mods + [key]))
        self.preview.set_text(self.captured)
        return True


class SettingsDialog(Gtk.Dialog):
    def __init__(self, app: "PanelApp"):
        super().__init__(title="jcode-panel Settings", transient_for=app.dropdown, flags=0)
        self.app = app
        add_class(self, "modern-dialog")
        self.set_default_size(560, 420)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("Save", Gtk.ResponseType.OK)

        notebook = Gtk.Notebook()
        notebook.set_margin_top(12)
        notebook.set_margin_bottom(12)
        notebook.set_margin_start(12)
        notebook.set_margin_end(12)

        grid = Gtk.Grid(column_spacing=10, row_spacing=10, margin=14)
        add_class(grid, "modern-card")
        self.hotkey = Gtk.Entry(text=normalize_hotkey(app.config.general.hotkey))
        self.hotkey.set_placeholder_text("f8 or ctrl+alt+j")
        hotkey_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        hotkey_box.pack_start(self.hotkey, True, True, 0)
        record_hotkey = Gtk.Button(label="Record…")
        record_hotkey.connect("clicked", self._record_hotkey)
        hotkey_box.pack_start(record_hotkey, False, False, 0)
        self.screenshot_hotkey = Gtk.Entry(text=normalize_hotkey(app.config.general.screenshot_hotkey))
        self.screenshot_hotkey.set_placeholder_text("ctrl+shift+s")
        screenshot_hotkey_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        screenshot_hotkey_box.pack_start(self.screenshot_hotkey, True, True, 0)
        record_screenshot_hotkey = Gtk.Button(label="Record…")
        record_screenshot_hotkey.connect("clicked", self._record_screenshot_hotkey)
        screenshot_hotkey_box.pack_start(record_screenshot_hotkey, False, False, 0)
        self.terminal = Gtk.Entry(text=app.config.general.terminal)
        self.template = Gtk.Entry(text=app.config.general.terminal_template)
        self.debug = Gtk.CheckButton(label="Debug raw preview")
        self.debug.set_active(app.config.general.debug)
        self.context = Gtk.CheckButton(label="Send context by default")
        self.context.set_active(app.config.session.send_context_default)
        self.auto_update = Gtk.CheckButton(label="Auto-update app on start")
        self.auto_update.set_active(app.config.general.auto_update_on_start)
        fields = [("Prompt hotkey", hotkey_box), ("Screenshot hotkey", screenshot_hotkey_box), ("Terminal", self.terminal), ("Terminal template", self.template)]
        for row, (label, widget) in enumerate(fields):
            field_label = Gtk.Label(label=label)
            field_label.set_xalign(0)
            grid.attach(field_label, 0, row, 1, 1)
            grid.attach(widget, 1, row, 1, 1)
        grid.attach(self.debug, 0, 4, 2, 1)
        grid.attach(self.context, 0, 5, 2, 1)
        grid.attach(self.auto_update, 0, 6, 2, 1)

        appearance = Gtk.Grid(column_spacing=10, row_spacing=10, margin=14)
        add_class(appearance, "modern-card")
        self.base_color = Gtk.Entry(text=app.config.ui.base_color)
        self.text_color = Gtk.Entry(text=app.config.ui.text_color)
        self.font_size = Gtk.SpinButton.new_with_range(10, 24, 1)
        self.font_size.set_value(app.config.ui.font_size)
        self.opacity = Gtk.SpinButton.new_with_range(0.35, 1.0, 0.05)
        self.opacity.set_digits(2)
        self.opacity.set_value(app.config.ui.floating_opacity)
        self.popup_width = Gtk.SpinButton.new_with_range(320, 1200, 10)
        self.popup_width.set_value(app.config.ui.popup_width)
        self.popup_height = Gtk.SpinButton.new_with_range(32, 220, 2)
        self.popup_height.set_value(app.config.ui.popup_height)
        self.bold = Gtk.CheckButton(label="Bold text")
        self.bold.set_active(app.config.ui.font_bold)
        self.italic = Gtk.CheckButton(label="Italic text")
        self.italic.set_active(app.config.ui.font_italic)
        appearance_fields = [
            ("Base color", self.base_color),
            ("Font color", self.text_color),
            ("Font size", self.font_size),
            ("Panel opacity", self.opacity),
            ("Popup width", self.popup_width),
            ("Popup slim height", self.popup_height),
        ]
        for row, (label, widget) in enumerate(appearance_fields):
            field_label = Gtk.Label(label=label)
            field_label.set_xalign(0)
            appearance.attach(field_label, 0, row, 1, 1)
            appearance.attach(widget, 1, row, 1, 1)
        appearance.attach(self.bold, 0, 6, 2, 1)
        appearance.attach(self.italic, 0, 7, 2, 1)
        hint = Gtk.Label(label="Use hex colors like #eff6ff. Changes apply after Save.")
        hint.set_xalign(0)
        hint.set_line_wrap(True)
        add_class(hint, "modern-subtitle")
        appearance.attach(hint, 0, 8, 2, 1)

        notebook.append_page(grid, Gtk.Label(label="General"))
        notebook.append_page(appearance, Gtk.Label(label="Appearance"))
        self.get_content_area().add(notebook)
        self.show_all()

    def _record_hotkey(self, _button):
        dialog = HotkeyCaptureDialog(self, self.hotkey.get_text())
        response = dialog.run()
        if response == Gtk.ResponseType.OK:
            self.hotkey.set_text(dialog.captured)
        dialog.destroy()

    def _record_screenshot_hotkey(self, _button):
        dialog = HotkeyCaptureDialog(self, self.screenshot_hotkey.get_text())
        response = dialog.run()
        if response == Gtk.ResponseType.OK:
            self.screenshot_hotkey.set_text(dialog.captured)
        dialog.destroy()

    def save(self):
        cfg = self.app.config
        old_hotkey = cfg.general.hotkey
        old_screenshot_hotkey = cfg.general.screenshot_hotkey
        cfg.general.hotkey = normalize_hotkey(self.hotkey.get_text().strip() or "f8")
        cfg.general.screenshot_hotkey = normalize_hotkey(self.screenshot_hotkey.get_text().strip() or "ctrl+shift+s")
        cfg.general.terminal = self.terminal.get_text().strip() or "auto"
        cfg.general.terminal_template = self.template.get_text().strip()
        cfg.general.debug = self.debug.get_active()
        cfg.general.auto_update_on_start = self.auto_update.get_active()
        cfg.session.send_context_default = self.context.get_active()
        cfg.ui.base_color = self.base_color.get_text().strip() or "#eff6ff"
        cfg.ui.text_color = self.text_color.get_text().strip() or "#1f2937"
        cfg.ui.font_size = int(self.font_size.get_value())
        cfg.ui.floating_opacity = float(self.opacity.get_value())
        cfg.ui.popup_width = int(self.popup_width.get_value())
        cfg.ui.popup_height = int(self.popup_height.get_value())
        cfg.ui.font_bold = self.bold.get_active()
        cfg.ui.font_italic = self.italic.get_active()
        cfg.save()
        reloaded = AppConfig.load()
        self.app.config = reloaded
        append_log(
            "Settings saved: "
            f"path={CONFIG_PATH} prompt={reloaded.general.hotkey} screenshot={reloaded.general.screenshot_hotkey} "
            f"base={reloaded.ui.base_color} text={reloaded.ui.text_color}"
        )
        if normalize_hotkey(old_hotkey) != cfg.general.hotkey or normalize_hotkey(old_screenshot_hotkey) != cfg.general.screenshot_hotkey:
            self.app.restart_hotkey_listener()


class PanelApp:
    def __init__(self):
        self.config = AppConfig.load()
        self.controller = AppController(self.config)
        self.control_server = ControlServer(self._handle_control)
        self.control_server.start()
        self.bridge = BrowserBridge()
        self.bridge.start()
        self.client = JcodeClient(self.controller.active_session)
        self.conversation = ConversationBuffer(self.config.ui.dropdown_max_messages)
        self.active_context = capture_active_context()
        self.process_status = "idle"
        self.live_activity = LiveActivity()
        self.activity_tick_id = 0
        self.send_watchdog_id = 0
        self.answer_timeout_id = 0
        self.send_sequence = 0
        self.answer_sequence = 0
        self.feedback_text = ""
        self.feedback_notice = ""
        self.pending_screenshots: list[str] = []
        self.capture_cancelled = False
        self.capture_in_progress = False
        self.capture_portal_done = False
        self.capture_portal_cancelled = False
        self.capture_existing_text = ""
        self.dropdown_refresh_id = 0
        self.last_prompt_toggle_at = 0.0
        self._ambient_shift = False
        self._ambient_ctrl = False
        self._ambient_alt = False
        self._hotkey_listener = None
        self._hotkey_pressed_mods: set[str] = set()
        self.indicator = AppIndicator3.Indicator.new("jcode-panel", "jcode-panel", AppIndicator3.IndicatorCategory.APPLICATION_STATUS)
        self.indicator.set_status(AppIndicator3.IndicatorStatus.ACTIVE)
        self.indicator.set_label("jcode", "")
        self.menu = Gtk.Menu()
        for label, cb in [("Open", self.toggle_dropdown), ("Prompt", self.show_prompt), ("Settings", self.show_settings), ("Update app", self.update_app), ("Quit", Gtk.main_quit)]:
            item = Gtk.MenuItem(label=label)
            item.connect("activate", lambda _i, c=cb: c())
            self.menu.append(item)
        self.menu.show_all()
        self.indicator.set_menu(self.menu)
        self.dropdown = Dropdown(self)
        self.floating = FloatingInput(self)
        self.toast = AnswerToast(self)
        if self.config.general.auto_update_on_start:
            self.update_app()
        self._warn_wayland_if_needed()
        self._add_system(f"Panel section: {self.controller.active_session_name}")
        if self.controller.active_session:
            self._add_system(f"Resuming jcode session: {self.controller.active_session}")
        else:
            self._add_system("No saved panel session yet. First prompt will create one and save it.")
        self._add_system(f"GTK backend: {os.environ.get('GDK_BACKEND', 'default')}")
        self._start_hotkey_listener()
        self._connect_jcode_async()
        notify("jcode-panel is running", "Use the top-bar icon, jcode-panel, or jcp to open it.")

    def _warn_wayland_if_needed(self):
        if os.environ.get("XDG_SESSION_TYPE", "").lower() == "wayland":
            self._add_system("Wayland detected: global hotkey and active-window context may be limited. v1 is X11-first.")

    def _start_hotkey_listener(self):
        self._stop_hotkey_listener()
        try:
            from pynput import keyboard  # type: ignore
        except Exception as exc:
            self._add_system(f"Global keyboard unavailable: {exc}")
            return

        hotkey = normalize_hotkey(self.config.general.hotkey)
        hotkey_mods, hotkey_key = hotkey_parts(hotkey)
        screenshot_hotkey = normalize_hotkey(self.config.general.screenshot_hotkey)
        screenshot_mods, screenshot_key = hotkey_parts(screenshot_hotkey)
        self.config.general.hotkey = hotkey
        self.config.general.screenshot_hotkey = screenshot_hotkey

        def combo_matches(normalized: str, wanted_key: str, wanted_mods: set[str]) -> bool:
            return bool(wanted_key) and normalized == wanted_key and wanted_mods.issubset(self._hotkey_pressed_mods)

        def on_press(key):
            normalized = key_name_from_pynput(key)
            if self.capture_in_progress and normalized == "escape":
                GLib.idle_add(self.cancel_capture_restore_prompt)
                return
            if normalized in MODIFIERS:
                self._hotkey_pressed_mods.add(normalized)
            elif combo_matches(normalized, screenshot_key, screenshot_mods):
                GLib.idle_add(self.capture_screenshot_for_prompt)
                return
            elif combo_matches(normalized, hotkey_key, hotkey_mods):
                GLib.idle_add(self.show_prompt)
                return
            GLib.idle_add(self._route_ambient_key, key, True)

        def on_release(key):
            normalized = key_name_from_pynput(key)
            if normalized in MODIFIERS:
                self._hotkey_pressed_mods.discard(normalized)
            GLib.idle_add(self._route_ambient_key, key, False)

        listener = keyboard.Listener(on_press=on_press, on_release=on_release)
        listener.daemon = True
        listener.start()
        self._hotkey_listener = listener
        self._add_system(f"Global keyboard active: prompt {hotkey}, screenshot {screenshot_hotkey}; ambient popup typing enabled")

    def _stop_hotkey_listener(self):
        listener = self._hotkey_listener
        self._hotkey_listener = None
        self._hotkey_pressed_mods.clear()
        if listener:
            try:
                listener.stop()
            except Exception:
                pass

    def restart_hotkey_listener(self):
        self._start_hotkey_listener()
        self._add_system(f"Hotkey updated to {self.config.general.hotkey}")

    def _connect_jcode_async(self):
        def worker():
            try:
                self.client.start_server()
                self.client.connect()
                threading.Thread(target=self.client.stream, args=(self.on_event,), daemon=True).start()
            except JcodeUnavailable as exc:
                append_log(f"jcode unavailable: {exc}")
                GLib.idle_add(self._add_system, f"jcode unavailable: {exc}. Open terminal to run setup/login.")
        threading.Thread(target=worker, daemon=True).start()

    def _add_system(self, text: str):
        append_log(text)
        self.conversation._append("system", text)
        self._schedule_dropdown_refresh(immediate=True)
        return False

    def on_event(self, event: PanelEvent):
        GLib.idle_add(self._on_event_ui, event)

    def _on_event_ui(self, event: PanelEvent):
        if event.kind == PanelEventKind.SESSION and event.session_id:
            old_session = self.controller.active_session
            self.controller.switch_session(event.session_id)
            self.client.adopt_session(event.session_id)
            if event.session_id != old_session:
                self.client.rename_session(event.session_id, self.controller.active_session_name)
                self._add_system(f"Saved panel section '{self.controller.active_session_name}' as {event.session_id}")
        self.conversation.add_event(event)
        self._schedule_dropdown_refresh()
        if event.kind in {PanelEventKind.STATUS, PanelEventKind.PROGRESS, PanelEventKind.TOOL}:
            if not self._handle_transient_status(event):
                self._record_activity_event(event)
            self._update_header_status()
            feedback = self._event_feedback_text(event)
            if feedback:
                self.feedback_text = feedback
                self.toast.update_feedback(self.feedback_text, self.feedback_notice)
            else:
                notice = self._event_notice_text(event)
                if notice:
                    self.feedback_notice = notice
                    self.toast.update_notice(notice)
        elif event.kind == PanelEventKind.ERROR:
            self._finish_activity("error")
            self.process_status = "error"
            self.feedback_text = event.text or "Error"
            self._update_header_status()
            self.toast.update_feedback(self.feedback_text, self.feedback_notice)
        elif event.kind == PanelEventKind.MESSAGE and event.text:
            if self.answer_sequence != self.send_sequence:
                return False
            terminal_message = activity_is_terminal(event)
            if event.raw and event.raw.get("type") == "done":
                # `done` repeats the full answer after text_delta chunks. Use it
                # only if no deltas were rendered.
                if not self.feedback_text:
                    self.feedback_text = event.text
            else:
                self.feedback_text += event.text
            self.process_status = "answering"
            notice = self._event_notice_text(event)
            if notice:
                self.feedback_notice = notice
            self._schedule_answer_timeout(self.answer_sequence)
            if terminal_message:
                self.stop_answering("complete")
            else:
                self.live_activity = LiveActivity(label="jcode", state="answering", started_at=self.live_activity.started_at or time.monotonic(), active=True)
                self._ensure_activity_tick()
            self._update_header_status()
            self.toast.update_feedback(self.feedback_text, self.feedback_notice)
        return False

    def _handle_transient_status(self, event: PanelEvent) -> bool:
        text = (event.text or "").strip().lower()
        if text.startswith("sending prompt"):
            self.process_status = "waiting for jcode"
            if self.live_activity.state == "sending":
                self._finish_activity("waiting")
            return True
        if text in {"jcode response complete", "message_end", "message end"}:
            self._finish_activity("complete")
            self.process_status = "complete"
            return True
        return False

    def _record_activity_event(self, event: PanelEvent):
        label = activity_label(event.raw, event.text or event.kind.value) or event.kind.value
        state = activity_state(event.raw, event.text or event.kind.value) or event.kind.value
        now = time.monotonic()
        if activity_is_terminal(event):
            self.process_status = state or label
            self._finish_activity(state or label)
            return
        if not self.live_activity.active or label != self.live_activity.label:
            self.live_activity = LiveActivity(label=label, state=state, started_at=now, active=True)
        else:
            self.live_activity.state = state
        self.process_status = state or label
        self._ensure_activity_tick()

    def _finish_activity(self, status: str = "idle"):
        self.live_activity.active = False
        self.live_activity.state = status or self.live_activity.state
        if self.activity_tick_id:
            GLib.source_remove(self.activity_tick_id)
            self.activity_tick_id = 0

    def stop_answering(self, status: str = "idle"):
        self.answer_sequence += 1
        if self.answer_timeout_id:
            GLib.source_remove(self.answer_timeout_id)
            self.answer_timeout_id = 0
        self._finish_activity(status)
        self.process_status = status or "idle"
        self._update_header_status()
        if status == "dismissed":
            self.toast.dismiss()

    def _schedule_answer_timeout(self, answer_sequence: int) -> None:
        if self.answer_timeout_id:
            GLib.source_remove(self.answer_timeout_id)
        self.answer_timeout_id = GLib.timeout_add_seconds(45, self._answer_timeout, answer_sequence)

    def _answer_timeout(self, answer_sequence: int) -> bool:
        self.answer_timeout_id = 0
        if answer_sequence == self.answer_sequence and self.live_activity.active and self.live_activity.state == "answering":
            self.stop_answering("timed out")
        return False

    def _ensure_activity_tick(self):
        if not self.activity_tick_id:
            self.activity_tick_id = GLib.timeout_add_seconds(1, self._activity_tick)

    def _activity_tick(self) -> bool:
        if not self.live_activity.active:
            self.activity_tick_id = 0
            return False
        self._update_header_status()
        return True

    def _update_header_status(self):
        if self.live_activity.active:
            elapsed = self._format_elapsed(self.live_activity.elapsed())
            label = f"{self.live_activity.state}: {self.live_activity.label} · {elapsed}"
        else:
            label = self.process_status.strip() or "idle"
        if len(label) > 62:
            label = label[:59] + "..."
        self.indicator.set_label(label, "")

    def _format_elapsed(self, seconds: int) -> str:
        minutes, secs = divmod(seconds, 60)
        if minutes:
            return f"{minutes}m{secs:02d}s"
        return f"{secs}s"

    def _event_feedback_text(self, event: PanelEvent) -> str:
        raw = event.raw or {}
        for key in ("feedback", "answer"):
            value = raw.get(key)
            if isinstance(value, str) and value.strip():
                return value.strip()
            if isinstance(value, dict):
                nested = PanelEvent(kind=event.kind, text="", raw=value)
                text = self._event_feedback_text(nested)
                if text:
                    return text
        ui = raw.get("ui")
        if isinstance(ui, dict):
            for key in ("feedback", "answer", "message"):
                value = ui.get(key)
                if isinstance(value, str) and value.strip():
                    return value.strip()
        return ""

    def _event_notice_text(self, event: PanelEvent) -> str:
        raw = event.raw or {}
        notices: list[str] = []
        if event.kind in {PanelEventKind.STATUS, PanelEventKind.PROGRESS, PanelEventKind.TOOL}:
            label = activity_label(raw, event.text or "")
            state = activity_state(raw, event.text or "")
            if label or state:
                notices.append(" · ".join(x for x in [state, label] if x))
        ctx = raw.get("context") or raw.get("active_context") or raw.get("context_summary")
        if isinstance(ctx, str) and ctx.strip():
            notices.append("context: " + ctx.strip())
        elif isinstance(ctx, dict):
            app = str(ctx.get("app") or ctx.get("window") or ctx.get("title") or "").strip()
            url = str(ctx.get("url") or "").strip()
            if app or url:
                notices.append("context: " + " · ".join(x for x in [app, url] if x))
        token_notice = token_notice_from_raw(raw)
        if token_notice:
            notices.append(token_notice)
        return "  •  ".join(dict.fromkeys(notices))

    def send_prompt(self, text: str, include_context: bool):
        self._sync_client_session()
        self.send_sequence += 1
        send_sequence = self.send_sequence
        self.answer_sequence = send_sequence
        if self.answer_timeout_id:
            GLib.source_remove(self.answer_timeout_id)
            self.answer_timeout_id = 0
        # Panel prompt should go to jcode mostly as typed. Screenshot chips shown
        # as [pic1] in the input are expanded to file paths only at send time.
        try:
            payload = expand_interaction_chips(self._expand_screenshot_chips(text.strip()))
        except InteractionContextError as exc:
            self._add_system(str(exc))
            self.process_status = "interaction unavailable"
            self.live_activity = LiveActivity(label="jcode", state="idle", started_at=time.monotonic(), active=False)
            self._update_header_status()
            self.show_prompt(text)
            self.floating.entry.set_placeholder_text(str(exc))
            return
        metadata = None
        self.pending_screenshots.clear()
        self.feedback_text = ""
        context_summary = self.active_context.summary() if include_context and self.active_context else ""
        self.feedback_notice = f"context: {context_summary}" if context_summary else ""
        self.process_status = "sending"
        self.live_activity = LiveActivity(label="jcode", state="sending", started_at=time.monotonic(), active=True)
        self._ensure_activity_tick()
        self._schedule_send_watchdog(send_sequence)
        self._update_header_status()
        self.conversation.add_user(text)
        self._schedule_dropdown_refresh(immediate=True)
        try:
            self.client.send(payload, metadata)
            self.controller.record_sent_prompt(text)
        except Exception as exc:
            self._add_system(str(exc))

    def _expand_screenshot_chips(self, text: str) -> str:
        def replace(match):
            index = int(match.group(1)) - 1
            if 0 <= index < len(self.pending_screenshots):
                return f"Screenshot file: {self.pending_screenshots[index]}"
            return match.group(0)
        return PIC_TAG_RE.sub(replace, text)


    def _schedule_send_watchdog(self, send_sequence: int) -> None:
        if self.send_watchdog_id:
            GLib.source_remove(self.send_watchdog_id)
        self.send_watchdog_id = GLib.timeout_add_seconds(8, self._send_watchdog, send_sequence)

    def _send_watchdog(self, send_sequence: int) -> bool:
        self.send_watchdog_id = 0
        if send_sequence != self.send_sequence:
            return False
        if self.live_activity.active and self.live_activity.state == "sending":
            self._finish_activity("waiting")
            self.process_status = "waiting for jcode"
            self._update_header_status()
        return False


    def _sync_client_session(self) -> None:
        """Keep the resident REPL aligned with the panel section state."""
        active = self.controller.active_session or ""
        if self.client.session_id != active or not self.client.process or self.client.process.poll() is not None:
            self.client.set_session(active)

    def _route_ambient_key(self, key, pressed: bool, force: bool = False):
        name = getattr(key, "name", None) or ""
        char = getattr(key, "char", None)
        lowered = str(name).lower()
        if lowered in {"shift", "shift_l", "shift_r"}:
            self._ambient_shift = pressed
            return False
        if lowered in {"ctrl", "ctrl_l", "ctrl_r", "control", "control_l", "control_r"}:
            self._ambient_ctrl = pressed
            return False
        if lowered in {"alt", "alt_l", "alt_r"}:
            self._ambient_alt = pressed
            return False
        if not pressed or not self.floating.get_visible():
            return False
        if self.floating.suppress_listener and not force:
            return False
        # If GTK successfully focused the entry, let normal GTK text handling
        # happen. Ambient routing is only the fallback when another app has focus.
        if self.floating.entry.has_focus():
            return False
        if lowered in {"enter", "return"}:
            self.floating.submit()
            return False
        if lowered == "esc":
            self.floating.hide()
            return False
        if lowered == "backspace":
            self.floating.backspace()
            return False
        if lowered == "space":
            self.floating.append_text(" ")
            return False
        if lowered == "tab":
            self._ambient_tab_complete()
            return False
        if char and not self._ambient_ctrl and not self._ambient_alt:
            self.floating.append_text(char)
        return False

    def _ambient_tab_complete(self):
        if self.floating._complete_interaction_at_cursor(add_trailing_space=True):
            return
        text = self.floating.entry.get_text()
        if text.startswith("/"):
            if not self.floating.completions.items:
                self.floating.completions.update(self.client.completions(text))
            suggestion = self.floating.completions.tab()
            if suggestion:
                self.floating.entry.set_text(suggestion)
                self.floating.entry.set_position(-1)

    def show_prompt(self, initial_text: str = ""):
        now = time.monotonic()
        # On X11/XWayland, F8 can arrive twice: once from GNOME custom
        # shortcut (`jcp`) and once from the internal pynput listener. Without
        # debouncing the popup opens then immediately closes.
        if now - self.last_prompt_toggle_at < 0.45:
            return
        self.last_prompt_toggle_at = now
        if self.floating.get_visible() and not initial_text:
            self.floating.hide()
        else:
            self.floating.show_at_pointer(initial_text)

    def cancel_input_mode(self) -> bool:
        self.capture_cancelled = True
        self.capture_in_progress = False
        self.capture_portal_done = True
        self.capture_portal_cancelled = True
        self.capture_existing_text = ""
        self.pending_screenshots.clear()
        self._finish_activity("cancelled")
        self.process_status = "cancelled"
        self._update_header_status()
        if self.floating.get_visible():
            self.floating.hide()
        return False

    def cancel_capture_restore_prompt(self) -> bool:
        if not self.capture_in_progress:
            return False
        existing_text = self.capture_existing_text
        self.capture_cancelled = True
        self.capture_in_progress = False
        self.capture_portal_done = True
        self.capture_portal_cancelled = True
        self._finish_activity("cancelled")
        self.process_status = "screenshot cancelled"
        self._update_header_status()
        self.show_prompt(existing_text)
        self.floating.entry.set_placeholder_text("Screenshot cancelled. Add text or try screenshot hotkey again.")
        return False

    def capture_screenshot_for_prompt(self) -> bool:
        """Hotkey flow: crop now, save image, then let user edit/send prompt."""
        if self.capture_in_progress:
            return False
        existing_text = self.floating.entry.get_text() if self.floating.get_visible() else ""
        self.capture_existing_text = existing_text
        self.capture_cancelled = False
        self.capture_in_progress = True
        self.capture_portal_done = False
        self.capture_portal_cancelled = False
        # The GNOME/portal drag UI cannot reliably receive events while our
        # XWayland popup owns focus/keyboard. Release it during selection, then
        # restore the input with the same content plus [pic#].
        if self.floating.get_visible():
            self.floating.hide()
        threading.Thread(target=self._capture_screenshot_for_prompt_worker, args=(existing_text,), daemon=True).start()
        GLib.timeout_add(450, self._show_capture_status_prompt, existing_text)
        GLib.timeout_add_seconds(10, self._capture_watchdog_restore, existing_text)
        return False

    def _show_capture_status_prompt(self, existing_text: str) -> bool:
        if self.capture_in_progress and not self.capture_cancelled and not self.floating.get_visible():
            self.show_prompt(existing_text)
            self.floating.entry.set_placeholder_text("Cropping screenshot... finish crop or press Esc to cancel.")
        return False

    def _capture_watchdog_restore(self, existing_text: str) -> bool:
        if self.capture_in_progress:
            append_log("Screenshot capture watchdog restored input popup")
            self.capture_cancelled = True
            self.capture_in_progress = False
            self.capture_portal_done = True
            self.capture_portal_cancelled = True
            self._restore_prompt_after_cancelled_capture(existing_text)
        return False

    def _capture_screenshot_for_prompt_worker(self, existing_text: str) -> None:
        screenshot_dir = Path(tempfile.gettempdir()) / "jcode-panel-screenshots"
        screenshot_dir.mkdir(parents=True, exist_ok=True)
        path = screenshot_dir / f"screenshot-area-{int(time.time())}.png"
        old_clipboard = self._clipboard_image_fingerprint(timeout=1.0)
        GLib.idle_add(self._set_capture_status, "screenshot area")
        trigger_done = threading.Event()

        def trigger_capture_ui():
            try:
                if not self.capture_in_progress or self.capture_cancelled:
                    return
                # Hotkey flow is clipboard-driven. Do not run legacy fallback
                # tools here, because they can open a second capture UI after
                # the clipboard watcher already restored the input popup.
                self._capture_portal_screenshot(path, interactive=True, timeout_seconds=8, stop_when_capture_done=True)
            finally:
                trigger_done.set()

        threading.Thread(target=trigger_capture_ui, daemon=True).start()
        captured = self._wait_for_clipboard_screenshot(path, old_clipboard, timeout_seconds=8)
        if not captured and path.exists() and path.stat().st_size > 0:
            captured = True
        append_log(f"Screenshot hotkey capture result: captured={captured} path={path} exists={path.exists()} size={path.stat().st_size if path.exists() else 0}")
        if captured:
            if self.capture_cancelled:
                GLib.idle_add(self.cancel_input_mode)
            else:
                GLib.idle_add(self._open_prompt_with_screenshot, str(path), existing_text)
            return
        GLib.idle_add(self._restore_prompt_after_cancelled_capture, existing_text)

    def _open_prompt_with_screenshot(self, path: str, existing_text: str = "") -> bool:
        self.capture_in_progress = False
        if self.capture_cancelled:
            return self.cancel_input_mode()
        self._finish_activity("captured")
        self.process_status = "screenshot ready"
        self._update_header_status()
        self.pending_screenshots.append(path)
        tag = pic_tag(len(self.pending_screenshots)) + " "
        base = existing_text.rstrip()
        new_text = ((base + " ") if base else "") + tag
        self.show_prompt(new_text)
        GLib.timeout_add(80, self._ensure_prompt_visible_with_text, new_text)
        GLib.timeout_add(220, self._ensure_prompt_visible_with_text, new_text)
        self.floating.entry.set_placeholder_text("Add text or more screenshots. Enter sends, Esc cancels request.")
        return False

    def _ensure_prompt_visible_with_text(self, text: str) -> bool:
        if not self.floating.get_visible():
            self.floating.show_at_pointer(text)
        elif not self.floating.entry.get_text().strip():
            self.floating.entry.set_text(text)
            self.floating.entry.set_position(-1)
        self.floating.present_with_time(Gtk.get_current_event_time())
        self.floating.entry.grab_focus()
        return False

    def _restore_prompt_after_cancelled_capture(self, existing_text: str) -> bool:
        self.capture_in_progress = False
        self._finish_activity("cancelled")
        self.process_status = "screenshot cancelled"
        self._update_header_status()
        if not self.capture_cancelled:
            self.show_prompt(existing_text)
            self.floating.entry.set_placeholder_text("Screenshot cancelled. Add text or try screenshot hotkey again.")
        return False

    def _show_capture_in_input_status(self, text: str) -> bool:
        if not self.floating.get_visible():
            self.show_prompt()
        self.floating.entry.set_placeholder_text(text)
        return False

    def _clipboard_image_fingerprint(self, timeout: float = 2.0) -> str:
        done = threading.Event()
        result = {"fingerprint": ""}

        def read_clipboard():
            try:
                image = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).wait_for_image()
                if image:
                    payload = image.get_pixels()
                    result["fingerprint"] = sha256(bytes(payload)).hexdigest()
            except Exception as exc:
                append_log(f"Clipboard image fingerprint failed: {exc}")
            finally:
                done.set()
            return False

        GLib.idle_add(read_clipboard)
        done.wait(timeout)
        return result["fingerprint"]

    def _save_clipboard_image(self, path: Path, timeout: float = 2.0) -> bool:
        done = threading.Event()
        result = {"ok": False}

        def save_clipboard():
            try:
                image = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).wait_for_image()
                if image:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    result["ok"] = bool(image.savev(str(path), "png", [], []))
            except Exception as exc:
                append_log(f"Clipboard image save failed: {exc}")
            finally:
                done.set()
            return False

        GLib.idle_add(save_clipboard)
        done.wait(timeout)
        return result["ok"] and path.exists() and path.stat().st_size > 0

    def _wait_for_clipboard_screenshot(self, path: Path, old_fingerprint: str, timeout_seconds: int = 30) -> bool:
        deadline = time.monotonic() + timeout_seconds
        while not self.capture_cancelled and time.monotonic() < deadline:
            current = self._clipboard_image_fingerprint(timeout=1.0)
            if current and current != old_fingerprint and self._save_clipboard_image(path, timeout=2.0):
                append_log(f"Screenshot captured from clipboard: {path}")
                return True
            if self.capture_portal_done and self.capture_portal_cancelled:
                append_log("Screenshot portal cancelled; stop clipboard wait")
                return False
            time.sleep(0.2)
        append_log(f"Clipboard screenshot wait timed out after {timeout_seconds}s")
        return False

    def handle_slash_command(self, text: str) -> bool:
        parts = text.strip().split(maxsplit=1)
        command = parts[0].lower()
        arg = parts[1].strip() if len(parts) > 1 else ""
        if command == "/model":
            if arg:
                self.set_model(arg)
            else:
                self.show_model_dialog()
            return True
        if command in {"/usage", "/ustage"}:
            self.show_usage_dialog()
            return True
        if command in {"/screen-shot", "/screenshot"}:
            # Keep one screenshot flow: capture first, then show [pic#] in the
            # input popup. Avoid the older direct-send screenshot worker because
            # it can leave a second capture UI running after Enter sends.
            GLib.timeout_add(120, self.capture_screenshot_for_prompt)
            return True
        if command in {"/help", "/?"}:
            self._show_text_dialog(
                "jcode-panel slash commands",
                "Popup commands:\n"
                "  /model            choose a model from a table\n"
                "  /model <name>     switch directly to a model\n"
                "  /usage, /ustage   show provider usage limits\n"
                "  /screen-shot            choose full screen or dragged area\n"
                "  /screen-shot full <q>   capture whole screen\n"
                "  /screen-shot area <q>   drag-select an area\n"
                "  /help             show this help\n\n"
                "Other slash commands are sent to jcode as normal prompts.",
            )
            return True
        return False

    def show_screenshot_mode_dialog(self, prompt: str = "Analyze this screenshot.") -> bool:
        dlg = Gtk.Dialog(title="Share screenshot", transient_for=self.dropdown, flags=0)
        add_class(dlg, "modern-dialog")
        dlg.add_button("Cancel", Gtk.ResponseType.CANCEL)
        area_btn = dlg.add_button("Drag area", 101)
        full_btn = dlg.add_button("Whole screen", 102)
        add_class(area_btn, "toast-icon-button")
        add_class(full_btn, "toast-icon-button")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, margin=14)
        add_class(box, "modern-card")
        title = Gtk.Label(label="What should jcode see?")
        title.set_xalign(0)
        add_class(title, "modern-title")
        subtitle = Gtk.Label(label="Choose a full-screen capture, or drag a rectangle on the screen. The image path is sent with your prompt.")
        subtitle.set_xalign(0)
        subtitle.set_line_wrap(True)
        add_class(subtitle, "modern-subtitle")
        prompt_label = Gtk.Label(label=f"Prompt: {prompt}")
        prompt_label.set_xalign(0)
        prompt_label.set_line_wrap(True)
        add_class(prompt_label, "toast-notice")
        box.pack_start(title, False, False, 0)
        box.pack_start(subtitle, False, False, 0)
        box.pack_start(prompt_label, False, False, 0)
        dlg.get_content_area().add(box)
        dlg.show_all()
        response = dlg.run()
        dlg.destroy()
        if response == 101:
            self.capture_screenshot_and_send(prompt, "area")
        elif response == 102:
            self.capture_screenshot_and_send(prompt, "full")
        return False

    def capture_screenshot_and_send(self, prompt: str = "Analyze this screenshot.", mode: str = "area") -> bool:
        mode = "full" if mode == "full" else "area"
        threading.Thread(target=self._capture_screenshot_worker, args=(prompt, mode), daemon=True).start()
        return False

    def _capture_screenshot_worker(self, prompt: str, mode: str) -> None:
        screenshot_dir = Path(tempfile.gettempdir()) / "jcode-panel-screenshots"
        screenshot_dir.mkdir(parents=True, exist_ok=True)
        path = screenshot_dir / f"screenshot-{mode}-{int(time.time())}.png"
        action = "Drag to select a screenshot area..." if mode == "area" else "Capturing whole screen..."
        GLib.idle_add(self.toast.update_feedback, action)
        GLib.idle_add(self._set_capture_status, f"screenshot {mode}")
        if mode == "area" and self._capture_gnome_shell_area(path):
            GLib.idle_add(self._send_screenshot_prompt, prompt, str(path))
            return
        if mode == "full" and self._capture_gnome_shell_full(path):
            GLib.idle_add(self._send_screenshot_prompt, prompt, str(path))
            return
        # Wayland/GNOME often denies direct full-screen screenshots to apps. The
        # desktop portal is the user-approved fallback and may show its own UI.
        if self._capture_portal_screenshot(path, interactive=True):
            GLib.idle_add(self._send_screenshot_prompt, prompt, str(path))
            return
        if self._capture_with_commands(path, mode):
            GLib.idle_add(self._send_screenshot_prompt, prompt, str(path))
            return
        GLib.idle_add(self._screenshot_failed, "Screenshot cancelled or no screenshot tool found")

    def _capture_with_commands(self, path: Path, mode: str) -> bool:
        commands = self._screenshot_commands(path, mode)
        error = ""
        for command in commands:
            try:
                result = self._run_screenshot_command(command)
                if result.returncode == 0 and path.exists() and path.stat().st_size > 0:
                    return True
                error = (result.stderr or "").strip() or f"{command[0]} exited {result.returncode}"
            except FileNotFoundError:
                continue
            except Exception as exc:
                error = str(exc)
        if error:
            append_log(f"Screenshot command capture failed: {error}")
        return False

    def _screenshot_commands(self, path: Path, mode: str) -> list[list[str]]:
        if mode == "area":
            return [
                ["gnome-screenshot", "-a", "-f", str(path)],
                ["grim", "-g", "$(slurp)", str(path)],
                ["scrot", "-s", str(path)],
                ["import", str(path)],
            ]
        return [
            ["gnome-screenshot", "-f", str(path)],
            ["grim", str(path)],
            ["scrot", str(path)],
            ["import", "-window", "root", str(path)],
        ]

    def _run_screenshot_command(self, command: list[str]) -> subprocess.CompletedProcess:
        # grim area selection uses slurp. It needs a shell for command substitution.
        if command[:2] == ["grim", "-g"] and "$(slurp)" in command:
            shell_cmd = f"grim -g \"$(slurp)\" {subprocess.list2cmdline([command[-1]])}"
            return subprocess.run(shell_cmd, shell=True, cwd=str(Path.home()), stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, timeout=60)
        return subprocess.run(command, cwd=str(Path.home()), stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, timeout=60)

    def _capture_portal_screenshot(self, path: Path, interactive: bool = True, timeout_seconds: int = 30, stop_when_capture_done: bool = False) -> bool:
        """Capture via xdg-desktop-portal Screenshot and copy returned URI."""
        try:
            bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
            done = threading.Event()
            result_uri = ""
            handle = ""
            token = f"jcode_panel_{int(time.time() * 1000)}"

            def on_response(_conn, _sender, _object_path, _iface, _signal, params):
                nonlocal result_uri
                if handle and _object_path != handle:
                    return
                if not handle and token not in str(_object_path):
                    return
                try:
                    response, results = params.unpack()
                    self.capture_portal_done = True
                    self.capture_portal_cancelled = int(response) != 0
                    if int(response) == 0:
                        uri = results.get("uri")
                        if uri:
                            result_uri = str(uri)
                finally:
                    done.set()

            sub_id = bus.signal_subscribe(
                "org.freedesktop.portal.Desktop",
                "org.freedesktop.portal.Request",
                "Response",
                None,
                None,
                Gio.DBusSignalFlags.NONE,
                on_response,
                None,
            )
            handle = bus.call_sync(
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Screenshot",
                "Screenshot",
                GLib.Variant("(sa{sv})", ("", {"interactive": GLib.Variant("b", bool(interactive)), "handle_token": GLib.Variant("s", token)})),
                GLib.VariantType.new("(o)"),
                Gio.DBusCallFlags.NONE,
                max(1000, timeout_seconds * 1000),
                None,
            ).unpack()[0]
            deadline = time.monotonic() + max(1, timeout_seconds)
            context = GLib.MainContext.default()
            try:
                while not done.is_set() and time.monotonic() < deadline:
                    if stop_when_capture_done and not self.capture_in_progress:
                        return path.exists() and path.stat().st_size > 0
                    while context.pending():
                        context.iteration(False)
                    done.wait(0.05)
                if not done.is_set():
                    append_log(f"Portal screenshot timed out after {timeout_seconds}s")
                    return False
            finally:
                bus.signal_unsubscribe(sub_id)
            if not result_uri:
                return False
            parsed = urlparse(result_uri)
            if parsed.scheme != "file":
                return False
            source = Path(unquote(parsed.path))
            if not source.exists() or source.stat().st_size <= 0:
                return False
            shutil.copyfile(source, path)
            return path.exists() and path.stat().st_size > 0
        except Exception as exc:
            append_log(f"Portal screenshot failed: {exc}")
            return False

    def _capture_gnome_shell_area(self, path: Path) -> bool:
        """Capture a user-selected area via GNOME Shell DBus.

        Ubuntu's Ctrl+Shift+A screenshot UI is GNOME Shell itself. Many installs
        no longer ship the `gnome-screenshot` CLI, so use the same shell DBus
        API directly.
        """
        try:
            select = subprocess.check_output(
                [
                    "gdbus",
                    "call",
                    "--session",
                    "--dest",
                    "org.gnome.Shell",
                    "--object-path",
                    "/org/gnome/Shell/Screenshot",
                    "--method",
                    "org.gnome.Shell.Screenshot.SelectArea",
                ],
                text=True,
                stderr=subprocess.STDOUT,
                timeout=60,
                cwd=str(Path.home()),
            )
            numbers = [int(part) for part in select.replace("(", " ").replace(")", " ").replace(",", " ").split() if part.lstrip("-").isdigit()]
            if len(numbers) < 4:
                return False
            x, y, width, height = numbers[:4]
            if width <= 0 or height <= 0:
                return False
            result = subprocess.check_output(
                [
                    "gdbus",
                    "call",
                    "--session",
                    "--dest",
                    "org.gnome.Shell",
                    "--object-path",
                    "/org/gnome/Shell/Screenshot",
                    "--method",
                    "org.gnome.Shell.Screenshot.ScreenshotArea",
                    str(x),
                    str(y),
                    str(width),
                    str(height),
                    "true",
                    str(path),
                ],
                text=True,
                stderr=subprocess.STDOUT,
                timeout=15,
                cwd=str(Path.home()),
            )
            return "true" in result.lower() and path.exists() and path.stat().st_size > 0
        except Exception as exc:
            append_log(f"GNOME Shell screenshot failed: {exc}")
            return False

    def _capture_gnome_shell_full(self, path: Path) -> bool:
        """Capture the whole screen via GNOME Shell DBus."""
        try:
            result = subprocess.check_output(
                [
                    "gdbus",
                    "call",
                    "--session",
                    "--dest",
                    "org.gnome.Shell",
                    "--object-path",
                    "/org/gnome/Shell/Screenshot",
                    "--method",
                    "org.gnome.Shell.Screenshot.Screenshot",
                    "true",   # include cursor
                    "true",   # flash area
                    str(path),
                ],
                text=True,
                stderr=subprocess.STDOUT,
                timeout=15,
                cwd=str(Path.home()),
            )
            return "true" in result.lower() and path.exists() and path.stat().st_size > 0
        except Exception as exc:
            append_log(f"GNOME Shell full screenshot failed: {exc}")
            return False

    def _set_capture_status(self, label: str) -> bool:
        self.process_status = label
        self.live_activity = LiveActivity(label=label, state="capturing", started_at=time.monotonic(), active=True)
        self._ensure_activity_tick()
        self._update_header_status()
        return False

    def _send_screenshot_prompt(self, prompt: str, path: str) -> bool:
        self._finish_activity("captured")
        message = f"{prompt.strip() or 'Analyze this screenshot.'}\n\nScreenshot file: {path}"
        self.toast.update_feedback(f"Screenshot captured: {path}")
        self.send_prompt(message, include_context=False)
        return False

    def _screenshot_failed(self, error: str) -> bool:
        self._finish_activity("screenshot failed")
        self.process_status = "screenshot failed"
        self._update_header_status()
        self._add_system(f"Screenshot failed: {error}")
        self.toast.update_feedback(f"Screenshot failed: {error}")
        return False

    def show_model_dialog(self):
        threading.Thread(target=self._load_models_and_show_dialog, daemon=True).start()

    def _load_models_and_show_dialog(self):
        try:
            raw = subprocess.check_output(["jcode", "model", "list", "--json"], text=True, stderr=subprocess.STDOUT, timeout=8, cwd=os.path.expanduser("~"))
            data = json.loads(raw)
            models = [str(x) for x in data.get("models", [])]
            selected = str(data.get("selected_model") or self.client.model or "")
            provider = str(data.get("provider") or "auto")
            GLib.idle_add(self._show_model_dialog_ui, models, selected, provider)
        except Exception as exc:
            GLib.idle_add(self._show_text_dialog, "jcode model", f"Could not load models:\n{exc}")

    def _show_model_dialog_ui(self, models: list[str], selected: str, provider: str):
        dlg = Gtk.Dialog(title=f"Choose jcode model ({provider})", transient_for=self.dropdown, flags=0)
        add_class(dlg, "modern-dialog")
        dlg.add_button("Cancel", Gtk.ResponseType.CANCEL)
        dlg.add_button("Use model", Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, margin=14)
        add_class(box, "modern-card")
        title = Gtk.Label(label="Choose a model")
        title.set_xalign(0)
        add_class(title, "modern-title")
        subtitle = Gtk.Label(label=f"Provider: {provider} · Current: {selected or 'default'}")
        subtitle.set_xalign(0)
        add_class(subtitle, "modern-subtitle")
        box.pack_start(title, False, False, 0)
        box.pack_start(subtitle, False, False, 0)

        store = Gtk.ListStore(str, str)
        active_iter = None
        for model in models:
            marker = "●" if model == selected else ""
            row = store.append([marker, model])
            if model == selected:
                active_iter = row
        tree = Gtk.TreeView(model=store)
        tree.set_headers_visible(True)
        tree.get_selection().set_mode(Gtk.SelectionMode.SINGLE)
        for title_text, column_index, width in [("", 0, 42), ("Model", 1, 520)]:
            renderer = Gtk.CellRendererText()
            column = Gtk.TreeViewColumn(title_text, renderer, text=column_index)
            column.set_min_width(width)
            tree.append_column(column)
        if active_iter is not None:
            tree.get_selection().select_iter(active_iter)
        scroller = Gtk.ScrolledWindow()
        scroller.set_min_content_width(620)
        scroller.set_min_content_height(360)
        scroller.add(tree)
        box.pack_start(scroller, True, True, 0)
        hint = Gtk.Label(label="Tip: type /model <name> to switch directly. New sends use this model.")
        hint.set_xalign(0)
        hint.set_line_wrap(True)
        add_class(hint, "modern-subtitle")
        box.pack_start(hint, False, False, 0)
        dlg.get_content_area().add(box)
        dlg.show_all()
        response = dlg.run()
        model = ""
        selected_model, selected_iter = tree.get_selection().get_selected()
        if selected_iter:
            model = str(selected_model[selected_iter][1])
        dlg.destroy()
        if response == Gtk.ResponseType.OK and model:
            self.set_model(model)
        return False

    def set_model(self, model: str):
        model = model.strip()
        if not model:
            return
        try:
            self.client.set_model(model)
            self.process_status = f"model: {model}"
            self._update_header_status()
            self._add_system(f"Selected jcode model: {model}")
            self.toast.update_feedback(f"Model switched to {model}")
        except Exception as exc:
            self._add_system(f"Model switch failed: {exc}")
            self.toast.update_feedback(f"Model switch failed: {exc}")

    def show_usage_dialog(self):
        threading.Thread(target=self._load_usage_and_show_dialog, daemon=True).start()

    def _load_usage_and_show_dialog(self):
        try:
            raw = subprocess.check_output(["jcode", "usage", "--json"], text=True, stderr=subprocess.STDOUT, timeout=12, cwd=os.path.expanduser("~"))
            data = json.loads(raw)
            rows: list[tuple[str, str, int, str, str]] = []
            for provider in data.get("providers", []):
                provider_name = str(provider.get("provider_name") or provider.get("name") or "provider")
                error = provider.get("error")
                if error:
                    rows.append((provider_name, "error", 0, str(error), ""))
                    continue
                for limit in provider.get("limits", []):
                    name = str(limit.get("name") or "limit")
                    pct = limit.get("usage_percent")
                    pct_value = 0 if pct is None else max(0, min(100, int(round(float(pct)))))
                    used = "unknown" if pct is None else f"{float(pct):.1f}%"
                    reset = str(limit.get("reset_in") or limit.get("resets_at") or "")
                    rows.append((provider_name, name, pct_value, used, reset))
                for key, value in provider.get("extra_info", []):
                    rows.append((provider_name, str(key), 0, str(value), ""))
            GLib.idle_add(self._show_usage_dialog_ui, rows)
        except Exception as exc:
            GLib.idle_add(self._show_text_dialog, "jcode usage", f"Could not load usage:\n{exc}")

    def _show_usage_dialog_ui(self, rows: list[tuple[str, str, int, str, str]]):
        dlg = Gtk.Dialog(title="jcode usage", transient_for=self.dropdown, flags=0)
        add_class(dlg, "modern-dialog")
        dlg.add_button("Close", Gtk.ResponseType.CLOSE)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, margin=14)
        add_class(box, "modern-card")
        title = Gtk.Label(label="Usage limits")
        title.set_xalign(0)
        add_class(title, "modern-title")
        subtitle = Gtk.Label(label="Current provider quota windows and reset times")
        subtitle.set_xalign(0)
        add_class(subtitle, "modern-subtitle")
        box.pack_start(title, False, False, 0)
        box.pack_start(subtitle, False, False, 0)
        store = Gtk.ListStore(str, str, int, str, str)
        for row in rows:
            store.append(list(row))
        tree = Gtk.TreeView(model=store)
        tree.set_headers_visible(True)
        for title_text, index, width in [("Provider", 0, 260), ("Window", 1, 180)]:
            renderer = Gtk.CellRendererText()
            column = Gtk.TreeViewColumn(title_text, renderer, text=index)
            column.set_min_width(width)
            tree.append_column(column)
        progress_renderer = Gtk.CellRendererProgress()
        progress_column = Gtk.TreeViewColumn("Usage", progress_renderer, value=2, text=3)
        progress_column.set_min_width(220)
        tree.append_column(progress_column)
        renderer = Gtk.CellRendererText()
        reset_column = Gtk.TreeViewColumn("Reset", renderer, text=4)
        reset_column.set_min_width(160)
        tree.append_column(reset_column)
        scroller = Gtk.ScrolledWindow()
        scroller.set_min_content_width(760)
        scroller.set_min_content_height(360)
        scroller.add(tree)
        box.pack_start(scroller, True, True, 0)
        dlg.get_content_area().add(box)
        dlg.show_all()
        dlg.run()
        dlg.destroy()
        return False

    def _show_text_dialog(self, title: str, text: str):
        dlg = Gtk.Dialog(title=title, transient_for=self.dropdown, flags=0)
        dlg.add_button("Close", Gtk.ResponseType.CLOSE)
        scroller = Gtk.ScrolledWindow()
        scroller.set_min_content_width(760)
        scroller.set_min_content_height(360)
        view = Gtk.TextView(editable=False, cursor_visible=False, monospace=True)
        view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        view.get_buffer().set_text(text)
        scroller.add(view)
        dlg.get_content_area().add(scroller)
        dlg.show_all()
        dlg.run()
        dlg.destroy()
        return False

    def _handle_control(self, command: str) -> ControlResponse:
        if command == "prompt":
            GLib.idle_add(self.show_prompt)
            return ControlResponse(True, "Toggled jcode-panel prompt")
        if command in {"show", "open"}:
            GLib.idle_add(self.show_dropdown)
            return ControlResponse(True, "Opened jcode-panel")
        if command == "status":
            return ControlResponse(True, "jcode-panel is running")
        if command == "quit":
            GLib.idle_add(Gtk.main_quit)
            return ControlResponse(True, "Quitting jcode-panel")
        return ControlResponse(False, f"Unknown command: {command}")

    def show_dropdown(self):
        self._schedule_dropdown_refresh(immediate=True)
        self.dropdown.show_all()
        self.dropdown.present()

    def _schedule_dropdown_refresh(self, immediate: bool = False):
        if immediate:
            if self.dropdown_refresh_id:
                GLib.source_remove(self.dropdown_refresh_id)
                self.dropdown_refresh_id = 0
            self.dropdown.refresh()
            return False
        if self.dropdown_refresh_id:
            return False
        # Avoid repainting the whole chat for every streamed token. If hidden,
        # defer work until the user opens the panel.
        delay = 80 if self.dropdown.get_visible() else 220
        self.dropdown_refresh_id = GLib.timeout_add(delay, self._flush_dropdown_refresh)
        return False

    def _flush_dropdown_refresh(self):
        self.dropdown_refresh_id = 0
        if self.dropdown.get_visible():
            self.dropdown.refresh()
        return False

    def toggle_dropdown(self):
        if self.dropdown.get_visible():
            self.dropdown.hide()
        else:
            self.show_dropdown()

    def open_terminal(self):
        self._sync_client_session()
        session = self.controller.active_session or ""
        if not session:
            self._add_system("No saved jcode session yet. Send the first panel prompt, then open the same session terminal.")
            self.show_prompt()
            return
        launch(f"jcode --resume {session}", self.config.general.terminal, self.config.general.terminal_template)

    def open_jcode(self):
        self.open_terminal()

    def new_session(self):
        name = self._ask_text("New jcode-panel section", "Section name", "")
        if name is None:
            return
        section_name = self.controller.start_new_section(name)
        self.client.set_session("")
        self.conversation = ConversationBuffer(self.config.ui.dropdown_max_messages)
        self.feedback_text = ""
        self.process_status = "idle"
        self._update_header_status()
        self._add_system(f"Started new panel section: {section_name}")
        self._add_system("First prompt will create and save a new Jcode session for this section.")
        self.dropdown.refresh()

    def resume_session(self):
        session = self._ask_text("Resume jcode-panel section", "Session id/name", self.controller.active_session)
        if session is None or not session.strip():
            return
        name = self._ask_text("Resume jcode-panel section", "Section display name", self.controller.active_session_name)
        if name is None:
            return
        self.controller.switch_session(session.strip(), name.strip() or session.strip())
        self.client.set_session(session.strip())
        self.conversation = ConversationBuffer(self.config.ui.dropdown_max_messages)
        self.feedback_text = ""
        self.process_status = "idle"
        self._update_header_status()
        self._add_system(f"Resumed panel section: {self.controller.active_session_name}")
        self._add_system(f"Jcode session: {self.controller.active_session}")
        self.dropdown.refresh()

    def _ask_text(self, title: str, label: str, default: str = "") -> str | None:
        dlg = Gtk.Dialog(title=title, transient_for=self.dropdown, flags=0)
        dlg.add_button("Cancel", Gtk.ResponseType.CANCEL)
        dlg.add_button("OK", Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin=12)
        box.pack_start(Gtk.Label(label=label), False, False, 0)
        entry = Gtk.Entry(text=default or "")
        entry.set_activates_default(True)
        box.pack_start(entry, False, False, 0)
        dlg.get_content_area().add(box)
        dlg.set_default_response(Gtk.ResponseType.OK)
        dlg.show_all()
        response = dlg.run()
        value = entry.get_text()
        dlg.destroy()
        if response != Gtk.ResponseType.OK:
            return None
        return value


    def update_app(self):
        result = self_update()
        self._add_system(result.message)

    def show_settings(self):
        dlg = SettingsDialog(self)
        response = dlg.run()
        if response == Gtk.ResponseType.OK:
            dlg.save()
            load_css(Gtk, Gdk, self.config)
        dlg.destroy()


def run_gtk_app(open_prompt: bool = False, open_dropdown: bool = False) -> int:
    load_css(Gtk, Gdk, AppConfig.load())
    app = PanelApp()
    if open_prompt:
        GLib.idle_add(app.show_prompt)
    if open_dropdown:
        GLib.idle_add(app.show_dropdown)
    Gtk.main()
    app.bridge.stop()
    app.control_server.stop()
    return 0
