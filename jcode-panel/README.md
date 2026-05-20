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
