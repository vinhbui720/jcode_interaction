from __future__ import annotations


def load_css(Gtk, Gdk) -> None:
    css = b'''
    * {
      font-family: Inter, Cantarell, Ubuntu, sans-serif;
      font-size: 13px;
    }
    window, dialog {
      background: transparent;
      color: #1f2937;
    }
    .panel-root {
      background: rgba(250, 252, 255, 0.80);
      border: 1px solid rgba(148, 163, 184, 0.42);
      border-radius: 18px;
      padding: 12px;
      box-shadow: 0 18px 46px rgba(15, 23, 42, 0.16);
    }
    .floating-root {
      background: transparent;
      border: none;
      padding: 0;
      margin: 0;
      box-shadow: none;
    }
    .context-strip {
      color: #2563eb;
      background: rgba(239, 246, 255, 0.80);
      border-radius: 9px;
      padding: 6px 8px;
      margin-bottom: 6px;
    }
    entry {
      color: #111827;
      background: rgba(255, 255, 255, 0.80);
      border: 1.5px solid rgba(96, 165, 250, 0.64);
      border-radius: 999px;
      padding: 12px 20px;
      min-height: 24px;
      caret-color: #2563eb;
      box-shadow: none;
    }
    entry:focus {
      border-color: #2563eb;
      background: rgba(255, 255, 255, 0.92);
      box-shadow: none;
    }
    textview, textview text {
      color: #1f2937;
      background: rgba(255, 255, 255, 0.62);
      font-family: JetBrains Mono, Fira Code, monospace;
      font-size: 12px;
    }
    scrolledwindow {
      border: 1px solid rgba(148, 163, 184, 0.34);
      border-radius: 12px;
      background: rgba(255, 255, 255, 0.56);
      padding: 8px;
    }

    .toast-root {
      background: rgba(250, 252, 255, 0.80);
      border: 1px solid rgba(148, 163, 184, 0.38);
      border-radius: 16px;
      padding: 12px;
      box-shadow: 0 18px 48px rgba(15, 23, 42, 0.16);
    }
    .toast-title {
      color: #2563eb;
      font-weight: 700;
      font-size: 12px;
    }
    .toast-text {
      color: #1f2937;
      background: transparent;
      font-size: 12px;
    }
    .toast-icon-button {
      min-width: 28px;
      min-height: 28px;
      padding: 5px;
      color: #1f2937;
      background: rgba(255, 255, 255, 0.56);
      border: 1px solid rgba(96, 165, 250, 0.24);
      border-radius: 999px;
    }
    .toast-icon-button:hover {
      background: rgba(219, 234, 254, 0.86);
      border-color: rgba(37, 99, 235, 0.42);
    }
    .toast-icon-button:active {
      background: rgba(147, 197, 253, 0.86);
      color: #0f172a;
    }

    button {
      color: #1f2937;
      background: rgba(255, 255, 255, 0.62);
      border: 1px solid rgba(148, 163, 184, 0.38);
      border-radius: 999px;
      padding: 7px 10px;
    }
    button:hover {
      background: rgba(219, 234, 254, 0.88);
      border-color: rgba(37, 99, 235, 0.42);
    }
    button:active {
      background: rgba(147, 197, 253, 0.92);
      color: #0f172a;
    }
    label {
      color: #1f2937;
    }
    checkbutton {
      color: #1f2937;
    }
    '''
    provider = Gtk.CssProvider()
    provider.load_from_data(css)
    screen = Gdk.Screen.get_default()
    if screen:
      Gtk.StyleContext.add_provider_for_screen(screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)


def add_class(widget, class_name: str) -> None:
    widget.get_style_context().add_class(class_name)
