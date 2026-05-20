from __future__ import annotations

import os
# Force XWayland/X11 on GNOME Wayland so floating prompt positioning works.
# Native Wayland intentionally prevents arbitrary window placement.
os.environ.setdefault("GDK_BACKEND", "x11")

import threading
import time
import subprocess
from dataclasses import dataclass

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("AppIndicator3", "0.1")
from gi.repository import AppIndicator3, GLib, Gtk, Gdk  # type: ignore

from .config import AppConfig
from .control import ControlResponse, ControlServer
from .services import AppController
from .context import BrowserBridge, capture_active_context
from .dropdown import ConversationBuffer
from .floating import CompletionState
from .diagnostics import append_log
from .jcode_client import JcodeClient, JcodeUnavailable
from .protocol import PanelEvent, PanelEventKind, activity_is_terminal, activity_label, activity_state
from .notify import notify
from .terminal import launch
from .style import add_class, load_css
from .positioning import xdotool_mouse_position_full
from .updater import self_update


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
        self.set_accept_focus(False)
        self.set_focus_on_map(False)
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
        self.add(box)
        self.set_default_size(520, 52)
        self.target_window_id = ""
        self.keyboard_grabbed = False

    def _draw_transparent(self, _widget, cr):
        cr.set_source_rgba(0, 0, 0, 0)
        cr.set_operator(0)  # cairo.OPERATOR_CLEAR without importing cairo
        cr.paint()
        cr.set_operator(2)  # cairo.OPERATOR_OVER
        return False

    def show_at_pointer(self):
        x, y, window_id = xdotool_mouse_position_full()
        self.target_window_id = window_id
        self.context_enabled = self.app.config.session.send_context_default
        self.entry.set_text("")
        self.entry.set_placeholder_text("Ask jcode...")
        self.typed_once = False
        self.follow_mouse = True
        self.current_x = None
        self.current_y = None

        self.show_all()
        self.realize()
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
        if self.entry.get_text():
            self.typed_once = True

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
        self.keyboard_grabbed = False

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
        if pos > 0:
            self.entry.set_text(current[:pos - 1] + current[pos:])
            self.entry.set_position(pos - 1)

    def submit(self) -> None:
        self._on_enter(self.entry)

    def _on_enter(self, _entry):
        text = self.entry.get_text().strip()
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
            self.hide()
            return True
        if alt and key and key.lower() == "c":
            self.context_enabled = not self.context_enabled
            return True
        if key == "Tab":
            text = self.entry.get_text()
            if text.startswith("/"):
                if not self.completions.items:
                    self.completions.update(self.app.client.completions(text))
                suggestion = self.completions.tab()
                if suggestion:
                    self.entry.set_text(suggestion)
                    self.entry.set_position(-1)
            return True
        return False


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
            ("⌨ Terminal", self.app.open_terminal),
            ("▣ jcode", self.app.open_jcode),
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
        self.label.set_xalign(0)
        self.label.set_line_wrap(True)
        self.label.set_max_width_chars(52)
        add_class(self.label, "toast-text")
        actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        open_btn = self._icon_button("view-list-symbolic", "Open conversation")
        open_btn.connect("clicked", lambda _b: self.app.show_dropdown())
        prompt_btn = self._icon_button("mail-reply-sender-symbolic", "Reply")
        prompt_btn.connect("clicked", lambda _b: self.app.show_prompt())
        close_btn = self._icon_button("window-close-symbolic", "Dismiss")
        close_btn.connect("clicked", lambda _b: self.hide())
        actions.pack_start(open_btn, False, False, 0)
        actions.pack_start(prompt_btn, False, False, 0)
        actions.pack_end(close_btn, False, False, 0)
        root.pack_start(title, False, False, 0)
        root.pack_start(self.label, True, True, 0)
        root.pack_start(actions, False, False, 0)
        self.add(root)
        self.set_default_size(380, 150)
        self.hide_source_id = 0
        self.refresh_source_id = 0
        self.pending_feedback = ""

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

    def update_feedback(self, text: str):
        text = text.strip()
        if not text:
            return
        self.pending_feedback = text[-900:]
        if not self.refresh_source_id:
            self.refresh_source_id = GLib.timeout_add(80, self._flush_feedback)
        self.show_all()
        self._move_to_corner()
        self._reset_idle_hide_timer()

    def _flush_feedback(self) -> bool:
        self.refresh_source_id = 0
        self.label.set_text(self.pending_feedback)
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


class SettingsDialog(Gtk.Dialog):
    def __init__(self, app: "PanelApp"):
        super().__init__(title="jcode-panel Settings", transient_for=app.dropdown, flags=0)
        self.app = app
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("Save", Gtk.ResponseType.OK)
        grid = Gtk.Grid(column_spacing=8, row_spacing=8, margin=12)
        self.hotkey = Gtk.Entry(text=app.config.general.hotkey)
        self.terminal = Gtk.Entry(text=app.config.general.terminal)
        self.template = Gtk.Entry(text=app.config.general.terminal_template)
        self.debug = Gtk.CheckButton(label="Debug raw preview")
        self.debug.set_active(app.config.general.debug)
        self.context = Gtk.CheckButton(label="Send context by default")
        self.context.set_active(app.config.session.send_context_default)
        self.auto_update = Gtk.CheckButton(label="Auto-update app on start")
        self.auto_update.set_active(app.config.general.auto_update_on_start)
        fields = [("Hotkey", self.hotkey), ("Terminal", self.terminal), ("Terminal template", self.template)]
        for row, (label, widget) in enumerate(fields):
            grid.attach(Gtk.Label(label=label), 0, row, 1, 1)
            grid.attach(widget, 1, row, 1, 1)
        grid.attach(self.debug, 0, 3, 2, 1)
        grid.attach(self.context, 0, 4, 2, 1)
        grid.attach(self.auto_update, 0, 5, 2, 1)
        self.get_content_area().add(grid)
        self.show_all()

    def save(self):
        cfg = self.app.config
        cfg.general.hotkey = self.hotkey.get_text().strip() or "f8"
        cfg.general.terminal = self.terminal.get_text().strip() or "auto"
        cfg.general.terminal_template = self.template.get_text().strip()
        cfg.general.debug = self.debug.get_active()
        cfg.general.auto_update_on_start = self.auto_update.get_active()
        cfg.session.send_context_default = self.context.get_active()
        cfg.save()


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
        self.feedback_text = ""
        self.dropdown_refresh_id = 0
        self.last_prompt_toggle_at = 0.0
        self._ambient_shift = False
        self._ambient_ctrl = False
        self._ambient_alt = False
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
        try:
            from pynput import keyboard  # type: ignore
        except Exception as exc:
            self._add_system(f"Global keyboard unavailable: {exc}")
            return

        hotkey = self.config.general.hotkey.lower()

        def on_press(key):
            name = getattr(key, "name", None) or getattr(key, "char", "")
            normalized = str(name).lower()
            if normalized == hotkey:
                GLib.idle_add(self.show_prompt)
                return
            GLib.idle_add(self._route_ambient_key, key, True)

        def on_release(key):
            GLib.idle_add(self._route_ambient_key, key, False)

        listener = keyboard.Listener(on_press=on_press, on_release=on_release)
        listener.daemon = True
        listener.start()
        self._add_system(f"Global keyboard active: {self.config.general.hotkey}; ambient popup typing enabled")

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
            self._record_activity_event(event)
            self._update_header_status()
        elif event.kind == PanelEventKind.ERROR:
            self._finish_activity("error")
            self.process_status = "error"
            self.feedback_text = event.text or "Error"
            self._update_header_status()
            self.toast.update_feedback(self.feedback_text)
        elif event.kind == PanelEventKind.MESSAGE and event.text:
            terminal_message = activity_is_terminal(event)
            if event.raw and event.raw.get("type") == "done":
                # `done` repeats the full answer after text_delta chunks. Use it
                # only if no deltas were rendered.
                if not self.feedback_text:
                    self.feedback_text = event.text
            else:
                self.feedback_text += event.text
            self.process_status = "answering"
            if terminal_message:
                self._finish_activity("answering")
            else:
                self.live_activity = LiveActivity(label="jcode", state="answering", started_at=self.live_activity.started_at or time.monotonic(), active=True)
                self._ensure_activity_tick()
            self._update_header_status()
            self.toast.update_feedback(self.feedback_text)
        return False

    def _record_activity_event(self, event: PanelEvent):
        label = activity_label(event.raw, event.text or event.kind.value) or event.kind.value
        state = activity_state(event.raw, event.text or event.kind.value) or event.kind.value
        now = time.monotonic()
        if activity_is_terminal(event):
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

    def send_prompt(self, text: str, include_context: bool):
        payload, metadata = self.controller.build_prompt(text, self.active_context, include_context)
        self.feedback_text = ""
        self.process_status = "sending"
        self.live_activity = LiveActivity(label="jcode", state="sending", started_at=time.monotonic(), active=True)
        self._ensure_activity_tick()
        self._update_header_status()
        self.conversation.add_user(text)
        self._schedule_dropdown_refresh(immediate=True)
        try:
            self.client.send(payload, metadata)
            self.controller.record_sent_prompt(text)
        except Exception as exc:
            self._add_system(str(exc))


    def _route_ambient_key(self, key, pressed: bool):
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
        text = self.floating.entry.get_text()
        if text.startswith("/"):
            if not self.floating.completions.items:
                self.floating.completions.update(self.client.completions(text))
            suggestion = self.floating.completions.tab()
            if suggestion:
                self.floating.entry.set_text(suggestion)
                self.floating.entry.set_position(-1)

    def show_prompt(self):
        now = time.monotonic()
        # On X11/XWayland, F8 can arrive twice: once from GNOME custom
        # shortcut (`jcp`) and once from the internal pynput listener. Without
        # debouncing the popup opens then immediately closes.
        if now - self.last_prompt_toggle_at < 0.45:
            return
        self.last_prompt_toggle_at = now
        if self.floating.get_visible():
            self.floating.hide()
        else:
            self.floating.show_at_pointer()

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
        session = self.controller.active_session or ""
        launch(f"jcode --resume {session}" if session else "jcode", self.config.general.terminal, self.config.general.terminal_template)

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
        dlg.destroy()


def run_gtk_app(open_prompt: bool = False, open_dropdown: bool = False) -> int:
    load_css(Gtk, Gdk)
    app = PanelApp()
    if open_prompt:
        GLib.idle_add(app.show_prompt)
    if open_dropdown:
        GLib.idle_add(app.show_dropdown)
    Gtk.main()
    app.bridge.stop()
    app.control_server.stop()
    return 0
