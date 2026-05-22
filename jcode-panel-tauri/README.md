# jcode-panel-tauri

Rust/Tauri migration branch for `jcode-panel`.

This is a real desktop app, not a browser tab. Tauri uses a native shell with embedded system webview windows.

## Goals

- Keep the current two-popup behavior: resident tray/dropdown + floating prompt.
- Keep persistent settings, token state, and resume session state stable across boot/restart.
- Keep VS Code and Obsidian integration refresh behavior.
- Make future development easier with small Rust and TypeScript modules.

## Setup

```bash
./setup.sh
```

If native Linux libs are missing, install:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

## Develop

```bash
npm run dev
```

## Current parity status

Implemented scaffold:
- Tauri tray app
- prompt popup window
- dropdown/status window
- modern settings window
- persistent config and state modules
- basic jcode command boundary
- VS Code and Obsidian integration installers

Still to port for full parity:
- global F8 registration
- exact mouse-adjacent popup positioning
- screenshot/crop flow
- streaming protocol parser and live activity UI
- complete jcode REPL/session adoption behavior
- browser integration bridge
