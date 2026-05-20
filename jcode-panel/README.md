# jcode-panel

Minimal Ubuntu/GNOME top-bar client for `jcode`.

MVP:
- F8 floating prompt
- saved default jcode session
- dropdown conversation preview
- context capture with optional browser extension bridge
- terminal handoff/resume
- settings backed by `~/.config/jcode-panel/config.toml`

Run:

```bash
python3 -m jcode_panel.main
```

Headless smoke check:

```bash
python3 -m jcode_panel.main --smoke
```

## jcode core integration

The panel now has a structured event contract so jcode core can enhance the GUI without the panel duplicating jcode logic.

See [`../docs/JCODE_PANEL_PROTOCOL.md`](../docs/JCODE_PANEL_PROTOCOL.md).

Preferred jcode output is JSON Lines such as:

```json
{"type":"panel.status","text":"Running tests..."}
{"type":"panel.progress","text":"Running tests","percent":42}
{"type":"panel.session","session_id":"fox"}
{"type":"panel.completions","items":[{"value":"/grill-me","kind":"skill"}]}
```

Plain text output still renders as a fallback.

## Launch readiness

Run diagnostics before launching:

```bash
PYTHONPATH=. python3 -m jcode_panel.main --diagnose
```

Logs are written to:

```text
~/.local/state/jcode-panel/jcode-panel.log
```

Install/autostart:

```bash
./install.sh
```

The installer does not install or configure jcode automatically. If diagnostics report jcode missing or not logged in, open a terminal and complete the normal jcode setup flow.

## Wayland / hotkey fallback

Ubuntu Wayland can block app-level global hotkeys from `pynput`. The installer now creates a GNOME-native custom shortcut:

- **F8 → `jcp` → open jcode-panel prompt**

If F8 still does not work, check Ubuntu Settings → Keyboard → Custom Shortcuts and make sure `jcode-panel Prompt` is bound to F8.

Fallbacks:

- tray menu → `Prompt`
- application launcher → `jcode-panel Prompt`
- command line: `PYTHONPATH=. python3 -m jcode_panel.main --prompt`

## Executor-friendly usage

After `./install.sh`, no terminal command is needed for normal use:

- App launcher: **jcode-panel**
- App launcher: **jcode-panel Prompt**
- Tray menu: **Prompt**
- Alias: `jcode-panel`
- Alias: `jcp` opens the prompt directly

## Self update

Source installs can update safely with a fast-forward only pull:

```bash
jcode-panel --self-update
```

The tray menu also has **Update app**. It never force-resets or deletes local work.

## Integrations

Integrations are structured as installable app adapters. Browser exists now; Obsidian scaffold exists for later.

```bash
jcode-panel --install-integration browser
jcode-panel --install-integration obsidian
```

Each integration owns its code under `integrations/<app>_plugin` or `extension/` and has a Python installer under `jcode_panel/integrations/`.

## Resident app behavior

`jcode-panel` is designed to run all the time:

- Login autostart runs `jcode-panel --background` quietly.
- The Ubuntu top-bar/header icon means the resident app is alive.
- Closing the dropdown only hides it; the app keeps working.
- `jcode-panel` opens the main dropdown/config UI of the running app.
- `jcp` opens the prompt of the running app.
- `jcode-panel --status` checks if it is alive.
- `jcode-panel --quit` intentionally stops the resident app.

## Floating prompt positioning

GNOME Wayland does not allow normal apps to place windows at arbitrary screen coordinates. To make the floating prompt follow the mouse, jcode-panel forces the GTK backend to X11/XWayland:

```text
GDK_BACKEND=x11
```

This is set automatically by the installed launchers and autostart entry.
