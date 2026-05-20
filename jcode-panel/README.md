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
