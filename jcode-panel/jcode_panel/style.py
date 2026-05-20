from __future__ import annotations


def load_css(Gtk, Gdk) -> None:
    css = b'''
    * {
      font-family: Inter, Cantarell, Ubuntu, sans-serif;
      font-size: 13px;
    }
    window, dialog {
      background: transparent;
      color: #cdd6f4;
    }
    .panel-root {
      background: #1e1e2e;
      border: 1px solid #45475a;
      border-radius: 14px;
      padding: 12px;
    }
    .floating-root {
      background: transparent;
      border: none;
      padding: 0;
      margin: 0;
      box-shadow: none;
    }
    .context-strip {
      color: #89b4fa;
      background: #181825;
      border-radius: 9px;
      padding: 6px 8px;
      margin-bottom: 6px;
    }
    entry {
      color: #cdd6f4;
      background: rgba(24, 24, 37, 0.96);
      border: 1.5px solid rgba(203, 166, 247, 0.72);
      border-radius: 999px;
      padding: 12px 20px;
      min-height: 24px;
      caret-color: #cba6f7;
      box-shadow: 0 16px 44px rgba(0,0,0,0.45), 0 0 0 1px rgba(255,255,255,0.04) inset;
    }
    entry:focus {
      border-color: #cba6f7;
      background: rgba(30, 30, 46, 0.98);
      box-shadow: 0 18px 52px rgba(0,0,0,0.56), 0 0 0 3px rgba(203,166,247,0.22);
    }
    textview, textview text {
      color: #cdd6f4;
      background: #181825;
      font-family: JetBrains Mono, Fira Code, monospace;
      font-size: 12px;
    }
    scrolledwindow {
      border: 1px solid #45475a;
      border-radius: 12px;
      background: #181825;
      padding: 8px;
    }
    button {
      color: #cdd6f4;
      background: #313244;
      border: 1px solid #45475a;
      border-radius: 10px;
      padding: 7px 10px;
    }
    button:hover {
      background: #45475a;
      border-color: #cba6f7;
    }
    button:active {
      background: #cba6f7;
      color: #181825;
    }
    label {
      color: #cdd6f4;
    }
    checkbutton {
      color: #cdd6f4;
    }
    '''
    provider = Gtk.CssProvider()
    provider.load_from_data(css)
    screen = Gdk.Screen.get_default()
    if screen:
      Gtk.StyleContext.add_provider_for_screen(screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)


def add_class(widget, class_name: str) -> None:
    widget.get_style_context().add_class(class_name)
