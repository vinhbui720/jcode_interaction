# Tauri migration plan

Branch: `rust/tauri`

Goals:
- Keep current resident-app behavior: tray, F8 prompt, dropdown, settings, jcode resume state.
- Preserve persistent config/state so boot/restart never resets session/token/settings accidentally.
- Use modular Rust backend and webview UI inside a real desktop app window, not a browser tab.

Modules:
- `core/config.rs`: persistent settings.
- `core/state.rs`: persistent runtime state, session, section, token stats.
- `core/jcode.rs`: jcode process boundary.
- `integrations/*`: VS Code and Obsidian installers/status.
- `ui/windows.rs`: prompt/dropdown/settings window control.
- `ui/tray.rs`: resident tray behavior.
- `ui/commands.rs`: Tauri command API.
- `src/*.ts`: small per-window frontend modules.

Current scaffold status:
- Two-window parity started: prompt popup + dropdown.
- Modern settings window scaffolded.
- Integration auto-refresh on startup started.
- Full feature parity still needs global hotkey, exact positioning, screenshots, protocol streaming, and full jcode REPL session adoption.
