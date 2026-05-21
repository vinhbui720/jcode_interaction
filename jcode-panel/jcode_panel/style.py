from __future__ import annotations


def _safe_hex(value: str, fallback: str) -> str:
    value = (value or "").strip()
    if len(value) == 7 and value.startswith("#") and all(c in "0123456789abcdefABCDEF" for c in value[1:]):
        return value
    return fallback


def _rgba(hex_color: str, alpha: float) -> str:
    value = _safe_hex(hex_color, "#eff6ff").lstrip("#")
    red = int(value[0:2], 16)
    green = int(value[2:4], 16)
    blue = int(value[4:6], 16)
    alpha = max(0.0, min(1.0, float(alpha)))
    return f"rgba({red}, {green}, {blue}, {alpha:.2f})"


def load_css(Gtk, Gdk, config=None) -> None:
    ui = getattr(config, "ui", None)
    base_color = _safe_hex(getattr(ui, "base_color", "#eff6ff"), "#eff6ff")
    text_color = _safe_hex(getattr(ui, "text_color", "#1f2937"), "#1f2937")
    opacity = max(0.35, min(1.0, float(getattr(ui, "floating_opacity", 0.92) or 0.92)))
    panel_bg = _rgba(base_color, opacity)
    font_size = max(10, min(24, int(getattr(ui, "font_size", 13) or 13)))
    font_weight = "700" if getattr(ui, "font_bold", False) else "400"
    font_style = "italic" if getattr(ui, "font_italic", False) else "normal"
    css = f'''
    * {{
      font-family: Inter, Cantarell, Ubuntu, sans-serif;
      font-size: {font_size}px;
      font-weight: {font_weight};
      font-style: {font_style};
    }}
    window, dialog {{
      background: transparent;
      color: {text_color};
    }}
    .panel-root {{
      background: {panel_bg};
      border: 1px solid rgba(148, 163, 184, 0.42);
      border-radius: 10px;
      padding: 10px;
      box-shadow: none;
    }}
    .floating-root {{
      background: transparent;
      border: none;
      padding: 0;
      margin: 0;
      box-shadow: none;
    }}
    .slash-hint {{
      color: #1e3a8a;
      background: rgba(239, 246, 255, 0.94);
      border: 1px solid rgba(96, 165, 250, 0.36);
      border-radius: 8px;
      padding: 6px 10px;
      margin-top: 4px;
      font-size: 12px;
      font-weight: 600;
      font-style: normal;
    }}
    .context-strip {{
      color: #2563eb;
      background: rgba(239, 246, 255, 0.80);
      border-radius: 6px;
      padding: 6px 8px;
      margin-bottom: 6px;
    }}
    entry {{
      color: #111827;
      background: rgba(255, 255, 255, 0.88);
      border: 1.5px solid rgba(96, 165, 250, 0.64);
      border-radius: 10px;
      padding: 7px 14px;
      min-height: 18px;
      caret-color: #2563eb;
      box-shadow: none;
    }}
    entry:focus {{
      border-color: #2563eb;
      background: rgba(255, 255, 255, 0.96);
      box-shadow: none;
    }}
    textview, textview text {{
      color: {text_color};
      background: rgba(255, 255, 255, 0.72);
      font-family: JetBrains Mono, Fira Code, monospace;
      font-size: 12px;
    }}
    scrolledwindow {{
      border: 1px solid rgba(148, 163, 184, 0.34);
      border-radius: 8px;
      background: rgba(255, 255, 255, 0.70);
      padding: 8px;
    }}

    .toast-root {{
      background: rgba(250, 252, 255, 0.84);
      border: 1px solid rgba(148, 163, 184, 0.38);
      border-radius: 10px;
      padding: 10px;
      box-shadow: none;
    }}
    .toast-title {{
      color: #2563eb;
      font-weight: 700;
      font-style: normal;
      font-size: 12px;
    }}
    .toast-text {{
      color: {text_color};
      background: transparent;
      font-size: 12px;
    }}
    .toast-notice {{
      color: #64748b;
      background: rgba(226, 232, 240, 0.55);
      border-left: 3px solid #38bdf8;
      border-radius: 7px;
      padding: 5px 8px;
      font-size: 11px;
      font-style: normal;
    }}
    .toast-icon-button {{
      min-width: 28px;
      min-height: 28px;
      padding: 5px;
      color: #1f2937;
      background: rgba(255, 255, 255, 0.70);
      border: 1px solid rgba(96, 165, 250, 0.24);
      border-radius: 10px;
    }}
    .toast-icon-button:hover {{
      background: rgba(219, 234, 254, 0.86);
      border-color: rgba(37, 99, 235, 0.42);
    }}
    .toast-icon-button:active {{
      background: rgba(147, 197, 253, 0.86);
      color: #0f172a;
    }}

    .modern-dialog {{
      background: #f8fafc;
      color: #0f172a;
      border-radius: 10px;
    }}
    .modern-card {{
      background: rgba(255, 255, 255, 0.98);
      border: 1px solid rgba(148, 163, 184, 0.34);
      border-radius: 10px;
      padding: 14px;
      color: #0f172a;
    }}
    .modern-title {{
      color: #0f172a;
      font-weight: 800;
      font-size: 18px;
      font-style: normal;
    }}
    .modern-subtitle {{
      color: #475569;
      font-size: 12px;
      font-style: normal;
    }}
    .hotkey-preview {{
      color: #1d4ed8;
      background: rgba(219, 234, 254, 0.92);
      border: 1px solid rgba(37, 99, 235, 0.30);
      border-radius: 8px;
      padding: 10px 12px;
      font-size: 18px;
      font-weight: 800;
      font-family: JetBrains Mono, Fira Code, monospace;
    }}
    notebook, notebook stack, notebook header {{
      background: #f8fafc;
      color: #0f172a;
    }}
    notebook tab {{
      background: #e2e8f0;
      color: #0f172a;
      border-radius: 6px 6px 0 0;
      padding: 8px 12px;
    }}
    notebook tab:checked {{
      background: #ffffff;
      color: #2563eb;
    }}
    treeview, treeview.view {{
      color: #0f172a;
      background: #ffffff;
      border-radius: 6px;
    }}
    treeview:selected, treeview.view:selected {{
      color: #ffffff;
      background: #2563eb;
    }}
    treeview header button {{
      color: #334155;
      background: #f8fafc;
      border-radius: 0;
      border: 0;
      border-bottom: 1px solid rgba(148, 163, 184, 0.4);
      font-weight: 700;
    }}
    levelbar block.filled {{
      background: #2563eb;
      border-radius: 6px;
    }}
    levelbar block.empty {{
      background: #dbeafe;
      border-radius: 6px;
    }}

    button {{
      color: #1f2937;
      background: rgba(255, 255, 255, 0.72);
      border: 1px solid rgba(148, 163, 184, 0.38);
      border-radius: 10px;
      padding: 7px 10px;
    }}
    button:hover {{
      background: rgba(219, 234, 254, 0.88);
      border-color: rgba(37, 99, 235, 0.42);
    }}
    button:active {{
      background: rgba(147, 197, 253, 0.92);
      color: #0f172a;
    }}
    label, checkbutton {{
      color: #1f2937;
    }}
    '''.encode()
    provider = Gtk.CssProvider()
    provider.load_from_data(css)
    screen = Gdk.Screen.get_default()
    if screen:
      Gtk.StyleContext.add_provider_for_screen(screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)


def add_class(widget, class_name: str) -> None:
    widget.get_style_context().add_class(class_name)
