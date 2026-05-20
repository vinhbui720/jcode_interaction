from __future__ import annotations

import os
# Force XWayland/X11 on GNOME Wayland so floating prompt positioning works.
# Native Wayland intentionally prevents arbitrary window placement.
os.environ.setdefault("GDK_BACKEND", "x11")

import threading

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
from .hotkeys import start_hotkey_listener
from .jcode_client import JcodeClient, JcodeUnavailable
from .protocol import PanelEvent, PanelEventKind
from .notify import notify
from .terminal import launch
from .style import add_class, load_css
from .positioning import xdotool_mouse_position
from .updater import self_update


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
        self.set_border_width(10)
        self.set_opacity(app.config.ui.floating_opacity)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        add_class(box, "floating-root")
        self.context_label = Gtk.Label(label="")
        add_class(self.context_label, "context-strip")
        self.context_label.set_xalign(0)
        self.entry = Gtk.Entry()
        self.entry.set_can_focus(True)
        self.entry.connect("activate", self._on_enter)
        self.entry.connect("key-press-event", self._on_key)
        self.entry.connect("changed", self._on_changed)
        box.pack_start(self.context_label, False, False, 0)
        box.pack_start(self.entry, False, False, 0)
        self.add(box)
        self.set_default_size(420, 72)

    def show_at_pointer(self):
        self.app.active_context = capture_active_context()
        self.context_enabled = self.app.config.session.send_context_default
        self.context_label.set_text("📎 " + self.app.active_context.summary())
        self.entry.set_text("")
        self.typed_once = False

        # GTK/Wayland often ignores move() before a window is mapped. Capture
        # the pointer first, show/realize the popup, then move it repeatedly on
        # idle so X11/XWayland has a chance to honor the coordinates.
        self.current_x = None
        self.current_y = None
        self.target_x = None
        self.target_y = None
        self.follow_mouse = True
        self.show_all()
        self.realize()
        self._follow_mouse_tick()
        if self.follow_source_id:
            GLib.source_remove(self.follow_source_id)
        self.follow_source_id = GLib.timeout_add(60, self._follow_mouse_tick)
        self.present()
        self.grab_focus()
        self.entry.grab_focus()
        GLib.idle_add(self._focus_entry)
        GLib.timeout_add(80, self._focus_entry)

    def _follow_mouse_tick(self) -> bool:
        if not self.follow_mouse or not self.get_visible():
            self.follow_source_id = 0
            return False
        x, y = self._mouse_position()
        if x is None or y is None:
            return True
        self.target_x = float(max(0, x + 20))
        self.target_y = float(max(0, y + 24))
        if self.current_x is None or self.current_y is None:
            self.current_x, self.current_y = self.target_x, self.target_y
        else:
            # Smooth easing avoids jitter but still tracks interaction.
            alpha = 0.28
            self.current_x += (self.target_x - self.current_x) * alpha
            self.current_y += (self.target_y - self.current_y) * alpha
            if abs(self.target_x - self.current_x) < 1:
                self.current_x = self.target_x
            if abs(self.target_y - self.current_y) < 1:
                self.current_y = self.target_y
        self.move(int(self.current_x), int(self.current_y))
        return True

    def _on_changed(self, _entry):
        # Once the user starts typing, stop following so the UI does not fight
        # text selection/cursor interaction. Empty prompt keeps following.
        if self.entry.get_text():
            self.typed_once = True
            self.follow_mouse = False

    def _focus_entry(self) -> bool:
        if self.get_visible():
            self.present()
            self.entry.grab_focus()
        return False

    def hide(self):
        self.follow_mouse = False
        self.typed_once = False
        if self.follow_source_id:
            GLib.source_remove(self.follow_source_id)
            self.follow_source_id = 0
        super().hide()

    def _mouse_position(self) -> tuple[int | None, int | None]:
        x, y = xdotool_mouse_position()
        if x is not None and y is not None:
            return x, y
        display = Gdk.Display.get_default()
        seat = display.get_default_seat() if display else None
        pointer = seat.get_pointer() if seat else None
        if pointer:
            _screen, x, y = pointer.get_position()
            return x, y
        return None, None

    def _on_enter(self, _entry):
        text = self.entry.get_text().strip()
        self.hide()
        if text:
            self.app.send_prompt(text, self.context_enabled)

    def _on_key(self, _widget, event):
        key = Gdk.keyval_name(event.keyval)
        alt = bool(event.state & Gdk.ModifierType.MOD1_MASK)
        # Any non-modifier key means the user is interacting with text; stop
        # mouse tracking so the window becomes stable and easy to type in.
        if key not in {"Shift_L", "Shift_R", "Control_L", "Control_R", "Alt_L", "Alt_R", "Super_L", "Super_R"}:
            self.follow_mouse = False
        if key == "Escape":
            self.hide()
            return True
        if alt and key and key.lower() == "c":
            self.context_enabled = not self.context_enabled
            prefix = "📎" if self.context_enabled else "🚫"
            self.context_label.set_text(prefix + " " + self.app.active_context.summary())
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
        lines = [f"{who}: {text}" for who, text in self.app.conversation.messages]
        self.buffer.set_text("\n\n".join(lines))


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
        if self.config.general.auto_update_on_start:
            self.update_app()
        self._warn_wayland_if_needed()
        self._add_system(f"GTK backend: {os.environ.get('GDK_BACKEND', 'default')}")
        self._start_hotkey_listener()
        self._connect_jcode_async()
        notify("jcode-panel is running", "Use the top-bar icon, jcode-panel, or jcp to open it.")

    def _warn_wayland_if_needed(self):
        if os.environ.get("XDG_SESSION_TYPE", "").lower() == "wayland":
            self._add_system("Wayland detected: global hotkey and active-window context may be limited. v1 is X11-first.")

    def _start_hotkey_listener(self):
        status = start_hotkey_listener(self.config.general.hotkey, lambda: GLib.idle_add(self.show_prompt))
        self._add_system(status.message)

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
        self.dropdown.refresh()
        return False

    def on_event(self, event: PanelEvent):
        GLib.idle_add(self._on_event_ui, event)

    def _on_event_ui(self, event: PanelEvent):
        if event.kind == PanelEventKind.SESSION and event.session_id:
            self.controller.switch_session(event.session_id)
        self.conversation.add_event(event)
        self.dropdown.refresh()
        self.indicator.set_label(self.conversation.latest_preview(self.config.general.debug), "")
        return False

    def send_prompt(self, text: str, include_context: bool):
        payload, metadata = self.controller.build_prompt(text, self.active_context, include_context)
        self.conversation.add_user(text)
        self.dropdown.refresh()
        try:
            self.client.send(payload, metadata)
            self.controller.record_sent_prompt(text)
        except Exception as exc:
            self._add_system(str(exc))

    def show_prompt(self):
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
        self.dropdown.show_all()
        self.dropdown.present()

    def toggle_dropdown(self):
        if self.dropdown.get_visible():
            self.dropdown.hide()
        else:
            self.show_dropdown()

    def open_terminal(self):
        session = self.controller.active_session or ""
        launch(f"jcode --resume {session}" if session else "jcode", self.config.general.terminal, self.config.general.terminal_template)

    def open_jcode(self):
        launch("jcode", self.config.general.terminal, self.config.general.terminal_template)

    def new_session(self):
        launch("jcode", self.config.general.terminal, self.config.general.terminal_template)

    def resume_session(self):
        launch("jcode --resume", self.config.general.terminal, self.config.general.terminal_template)


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
