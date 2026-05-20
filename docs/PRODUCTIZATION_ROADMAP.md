# jcode-panel Productization Roadmap

Goal: turn the current MVP into a reliable, installable, native-feeling jcode desktop client.

## Current status

The repository contains a working MVP shell:

- GTK/AppIndicator app shell
- F8 floating input
- dropdown conversation buffer
- context capture via X11 and optional browser bridge
- jcode subprocess client wrapper
- settings TOML and basic settings dialog
- terminal launcher adapters
- smoke tests and dependency-free unit tests

## Production gaps

### P0: Correct jcode integration contract

- Detect actual supported jcode transport: formal API, `jcode connect`, or another command.
- Implement a transport abstraction instead of coupling UI directly to subprocess details.
- Persist real session ID/name returned by jcode, not just a user-entered string.
- Support reconnect/backoff and clear disconnected states.
- Add structured event parsing tests using real/sample jcode events.

### P0: Stable session/state layer

- Separate mutable runtime state from user config.
- Store saved session, last prompt history, window state, and bridge status in `state.toml`.
- Make `+ New`, `Resume`, and first-launch session creation update the saved default reliably.

### P0: Testable architecture

- Decouple GTK widgets from jcode client, config, terminal launcher, and context capture.
- Add service objects: `AppState`, `JcodeService`, `ContextService`, `CompletionService`, `TerminalService`.
- Add tests for state persistence, terminal adapters, completion cycling, prompt construction, and error states.

### P1: UX completeness

- Real settings dialog with validation and hotkey rebinding lifecycle.
- Native-like completion popup, not just Tab replacement.
- Prompt history cycling and persistence.
- Context reveal/expand UI.
- Top-bar preview filtering for progress/error/debug modes.
- Better missing-jcode/login/setup screen.

### P1: Packaging/install

- Debian/Ubuntu dependency check with actionable errors.
- `pipx`/wheel install path.
- `.desktop` entry for launching app manually and autostart.
- Browser extension install docs for Chrome/Firefox.
- Logging to `~/.local/state/jcode-panel/jcode-panel.log`.

### P2: Platform robustness

- X11-first polish.
- Wayland compatibility warning and graceful degradation.
- Optional portal/DBus implementation later.

## Definition of full-product ready

1. Fresh clone can install with one documented command path.
2. App starts without crashing when jcode is missing, logged out, disconnected, or slow.
3. First launch creates/connects a session and persists it.
4. Reopen resumes the saved session.
5. F8 prompt sends context-aware messages.
6. Dropdown streams structured events or markdown fallback.
7. Terminal handoff works for common terminal emulators.
8. Tests cover core logic without GTK, plus smoke validation for CLI entrypoint.
9. Code has clear module boundaries and no UI-only business logic.
