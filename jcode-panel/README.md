# jcode-panel

Minimal Ubuntu/GNOME top-bar client for `jcode`.

MVP:
- F8 floating prompt
- saved default jcode session
- dropdown conversation preview
- context capture with optional browser/VS Code/Obsidian integration bridge
- terminal handoff/resume
- settings backed by `~/.config/jcode-panel/config.toml`

## Fresh install on another Ubuntu/GNOME machine

```bash
# 1) Install system deps
sudo apt update
sudo apt install -y git python3 python3-pip python3-gi gir1.2-appindicator3-0.1 gir1.2-gtk-3.0 xdotool x11-utils xclip xsel

# 2) Install Python deps used by the resident hotkey/context helpers
python3 -m pip install --user pynput requests

# 3) Clone and install this repo
git clone git@github.com:vinhbui720/jcode_interaction.git
cd jcode_interaction/jcode-panel
./install.sh

# 4) Start/restart the resident app
jcode-panel --quit || true
jcode-panel --background &

# 5) Check health
jcode-panel --status
PYTHONPATH=. python3 -m jcode_panel.main --diagnose
```

Requirements:
- `jcode` CLI must already be installed and logged in.
- `~/.local/bin` should be in `PATH` so `jcode-panel` and `jcp` work.
- On GNOME Wayland the app launchers force `GDK_BACKEND=x11` for floating prompt positioning.

Normal use after install:
- **F8** opens the floating prompt.
- `jcp` opens the prompt.
- `jcode-panel` opens the dropdown/config UI.
- `jcode-panel --quit` stops the resident app.

## Install integrations

Install only the integrations you use:

```bash
jcode-panel --install-integration vscode
jcode-panel --install-integration obsidian
jcode-panel --install-integration browser
```

### VS Code

The installer copies the local extension from `integrations/vscode_extension/`. Reload VS Code after install. The extension writes context only when the editor, cursor, or selection changes:

```text
~/.local/state/jcode-panel/contexts/vscode.json
```

It records active file path, line/column, workspace, language, and selected text.

### Obsidian

The installer copies the plugin from `integrations/obsidian_plugin/` into your vault plugin directory. Enable it in Obsidian Community plugins, then reload Obsidian. It writes:

```text
~/.local/state/jcode-panel/contexts/obsidian.json
```

If using the AppImage, keep launching Obsidian normally. Example local path used on this machine:

```text
~/Desktop/obsidian/Obsidian-1.10.6.AppImage
```

### Browser

The browser bridge is optional. Install the extension from `extension/` or `integrations/browser_extension/` depending on browser flow. It sends active tab/selection metadata to the resident app.

## Floating prompt interaction

### App context chips

The prompt supports explicit app tags:

```text
@vscode fix this bug
compare @vscode with @obsidian
```

Known tags become editable chips like `[@vscode]` and `[@obsidian]`. Partial tokens such as `@vsc` or `@obs` show hints and can be converted with Tab, Space, or Enter. Slash `/...` remains reserved for jcode commands such as `/screen-shot`.

On submit, each chip expands into an on-demand context block and the remaining prompt text is sent to jcode. If an app has no active context, the prompt is not sent and the panel asks you to re-input. Backspace at the end of a chip removes the whole chip.

- `@vscode`: active file, cursor line, selection if any, surrounding code, workspace root, and nearby symbol/import hints.
- `@obsidian`: active note path/title, cursor line, selection if any, or nearby note excerpt.

### Focused-window auto chips

When the floating prompt opens, it may seed chips from the focused window only:

- selected text -> `[text:file:line]`
- selected URL -> `[link:domain]`
- selected files, when a focused detector provides paths -> `[file:name]` or `[n files]`

Clipboard is **not** used for auto-chip creation. Clipboard only enters the prompt when the user explicitly presses `Ctrl+V`.

### Prompt input details

- Right side counter shows `#/4000` for current input length.
- Counter turns warning color after 4000 chars.
- `Ctrl+V` pastes clipboard text into the input.
- Multiline paste is flattened to one line for reliable `Gtk.Entry` cursor behavior.
- Screenshot chips like `[pic1]` are expanded to screenshot file paths at send time.

## Run from source

```bash
cd jcode_interaction/jcode-panel
PYTHONPATH=. python3 -m jcode_panel.main
```

Open prompt from source:

```bash
PYTHONPATH=. python3 -m jcode_panel.main --prompt
```

Headless smoke check:

```bash
PYTHONPATH=. python3 -m jcode_panel.main --smoke
```

Run tests:

```bash
python3 run_tests.py
```

## jcode core integration

The panel has a structured event contract so jcode core can enhance the GUI without the panel duplicating jcode logic.

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

Ubuntu Wayland can block app-level global hotkeys from `pynput`. The installer creates a GNOME-native custom shortcut:

- **F8 → `jcp` → open the Jcode Interaction prompt**

If F8 still does not work, check Ubuntu Settings → Keyboard → Custom Shortcuts and make sure `Jcode Interaction` is bound to F8.

Fallbacks:

- tray menu → `Prompt`
- application launcher → `Jcode Interaction`
- command line: `PYTHONPATH=. python3 -m jcode_panel.main --prompt`

## Executor-friendly usage

After `./install.sh`, no terminal command is needed for normal use:

- App launcher: **Jcode Interaction**
- Tray menu: **Prompt**
- Alias: `jcode-panel`
- Alias: `jcp` opens the prompt directly

## Self update

Source installs can update safely with a fast-forward only pull:

```bash
jcode-panel --self-update
```

The tray menu also has **Update app**. It never force-resets or deletes local work.

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
