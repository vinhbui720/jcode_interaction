# jcode-panel — Product Specification

> A minimal, modern GUI client for jcode that lives in the Ubuntu top bar.  
> Lightweight by design. Powerful by nature — it's a full jcode client under the hood.

---

## 1. Concept

jcode-panel is a **native Ubuntu panel application** that acts as a first-class GUI client of the jcode server. It does not reimplement jcode logic. Instead, it connects to the jcode server the same way the CLI does — inheriting all features: sessions, memory, swarm, multiagent, MCP, and more.

The guiding rule: **if jcode can do it natively, delegate to jcode natively.** The panel never duplicates what jcode already does — it just provides a lightweight GUI surface to reach it faster.

The UI is intentionally **minimal and modern**: no bloat, no heavy chrome. Think of it as a whisper-quiet assistant sitting in your top bar, ready when you need it, invisible when you don't.

---

## 2. Core Philosophy

| Principle | Meaning |
|---|---|
| **GUI is a client** | All intelligence lives in jcode. The panel just talks to it. |
| **Native first** | Any feature jcode supports natively — open it natively, don't wrap it. |
| **Minimal surface** | Small footprint — visually and in RAM. |
| **Non-intrusive** | Out of the way until summoned. |
| **Context-aware** | Knows what you're doing without you explaining. |
| **Session continuity** | Close the panel, session lives on. Resume anytime — in panel or terminal. |

---

## 3. UI Components

### 3.1 Top Bar Indicator

- A small icon (🤖 or custom minimal icon) always visible in the Ubuntu top bar (GNOME panel / AppIndicator3).
- Next to the icon, a **one-line preview** of the most recent jcode message, truncated gracefully.
- Two small action buttons inline:
  - `≡` — open the dropdown panel
  - `✕` — dismiss / minimize

```
┌─────────────────────────────────────────────────────┐
│  Ubuntu Top Bar                                      │
│  🤖  "done — 3 files updated"      [≡]  [✕]         │
└─────────────────────────────────────────────────────┘
```

---

### 3.2 Dropdown Panel (Conversation View)

Opens when the indicator icon or `≡` is clicked. Appears anchored just below the top bar.

**Design:**
- Compact chat-style layout — like a messenger window, not a terminal.
- Soft dark background, subtle border radius, no harsh borders.
- Scrollable conversation history (recent messages only — last N turns).
- Each message is clearly attributed: `You` vs `jcode`.
- Streaming output — jcode's response appears word by word as it streams in.
- Auto-scroll to the latest message.

**Bottom action bar (inside dropdown):**

| Button | Action |
|---|---|
| `⌨ Open in Terminal` | Opens the configured terminal and runs `jcode --resume <session-id>` — full native jcode TUI for the current session |
| `▣ Open jcode` | Launches jcode natively in terminal (full TUI, not a wrapper) |
| `+ New Session` | Runs `jcode` in a new terminal tab/window — native fresh session |
| `↩ Resume` | Opens session picker; selected session opens as `jcode --resume <name>` in terminal |

```
┌────────────────────────────────┐
│  💬 You: fix the login bug     │
│  🤖 Done. Changed auth.py L42  │
│  💬 You: explain the change    │
│  🤖 The issue was a missing... │  ← streams in live
│                                │
│  [⌨ Terminal] [▣ jcode] [+] [↩]│
└────────────────────────────────┘
```

> **Note:** There is no separate "Full View" window. Any need for a larger view is handled by opening jcode natively in a terminal — that's the full experience, exactly as intended.

---

### 3.3 Floating Input Box

The primary way to send prompts — especially while working in another app.

**Trigger:** Global hotkey (default: `F8`, user-configurable in Settings)

**Behavior:**
- Appears instantly at the **current mouse cursor position**.
- A single-line text input field — nothing else.
- Minimal design: rounded corners, soft shadow, semi-transparent dark background.
- Auto-focuses — start typing immediately, no click needed.
- Shows a **context strip** at the top (one line, read-only) indicating what jcode will know:

```
┌──────────────────────────────────────┐
│ 📎 Firefox · github.com/user/repo    │  ← auto context
│ > _                                  │  ← cursor ready
└──────────────────────────────────────┘
```

**Key bindings inside floating box:**

| Key | Action |
|---|---|
| `Enter` | Send prompt to jcode, close floating box |
| `Esc` | Dismiss floating box, do nothing |
| `Alt+V` | Switch to voice input mode (stub — ready for later) |
| `Alt+C` | Toggle captured context on/off for this prompt |
| `↑ / ↓` | Cycle through recent prompt history |
| `Tab` | Accept current completion suggestion; repeated Tab cycles related suggestions |

**After Enter:**
- Floating box disappears immediately.
- Prompt appears in the dropdown conversation.
- jcode response streams in.
- Top bar preview updates with the latest message.

**Autocomplete:**
- Slash commands and skills should feel like native jcode CLI completion.
- Typing `/` shows available slash-command suggestions.
- Typing a prefix such as `/gr` narrows suggestions, e.g. `/grill-me`.
- `Tab` accepts the current suggestion; repeated `Tab` cycles through related suggestions.
- Completion data must come from jcode when possible: first via a formal completion API, then via a CLI completion fallback. The panel must not hardcode the command list except as an emergency degraded mode.

**Context visibility:**
- The context strip shows a compact human-readable summary by default.
- The user can expand/reveal the exact context that will be sent.
- The user can disable context for the current prompt with `Alt+C`.

---

### 3.4 Session Picker (Modal)

A small modal that appears when `↩ Resume` is pressed.

- Lists recent jcode sessions by memorable name and last-used timestamp.
- Selecting a session opens it natively: runs `jcode --resume <name>` in the configured terminal.
- Keyboard navigable (arrow keys + Enter).
- Pressing Esc cancels without action.

```
┌─────────────────────────────────┐
│  Resume a session               │
│                                 │
│  > fox        2 min ago         │
│    rabbit     1 hour ago        │
│    owl        yesterday         │
│                                 │
│  [↩ Open in Terminal]   [✕]     │
└─────────────────────────────────┘
```

---

## 4. Context Awareness

When the floating input box opens, the app **silently captures** the current context and attaches it to every prompt:

| Context Item | How it's captured |
|---|---|
| Active application name | `xdotool getactivewindow` + `WM_CLASS` |
| Active window title | `xdotool getwindowname` |
| Browser tab URL | Browser extension (Chrome/Firefox) → local HTTP bridge |
| Browser tab title | Same bridge |
| Browser selected text | Same bridge, when present |

The captured context is attached to the user's prompt automatically, so jcode understands what the user is looking at without the user having to explain.

Preferred behavior is to send context as structured hidden metadata through the jcode client protocol. If that is not available, the panel falls back to prepending a compact visible context block:

```text
[Context]
App: Firefox
Title: GitHub issue ...
URL: https://...
Selected text: ...
[/Context]

User prompt...
```

The context strip shown in the floating box is a **human-readable summary** of what was captured (e.g. `Firefox · github.com/user/repo`). It is not editable — it's informational only.

---

## 5. Session Management

The panel behaves like a full jcode CLI client for session control. Wherever possible, session actions are handed off to jcode natively in a terminal — not handled inside the panel GUI.

| Action | Behavior |
|---|---|
| Panel opens (first time) | Starts `jcode serve` if not running, creates a new jcode client/session, then persists that session ID/name |
| Panel reopens | Auto-reconnects/resumes the saved default panel session |
| `+ New Session` | Creates/opens a new native jcode session and makes it the saved default |
| `↩ Resume` | Opens session picker → selected session runs `jcode --resume <name>` in terminal and becomes the saved default |
| `⌨ Open in Terminal` | Opens configured terminal and attaches current session via `jcode --resume <session-id>` |
| `▣ Open jcode` | Opens terminal with plain `jcode` — native TUI, full experience |
| Panel closed | Session remains alive on the jcode server |
| jcode server stopped | Panel detects disconnect and shows a reconnect prompt |

The panel and terminal may attach to and control the same jcode session concurrently. The panel is a real jcode client, not a log viewer.

**Key principle:** The panel's dropdown is for **glancing and quick prompts**. Anything deeper — long sessions, file editing, swarm management — belongs in the native jcode TUI opened in a terminal.

---

## 6. Voice Input (Ready, Not Active)

- `Alt+V` inside the floating box switches to voice mode.
- A mic icon replaces the text cursor — visual indicator only for now.
- The underlying hook is in place for a future STT (speech-to-text) integration.
- When implemented: spoken input → transcribed → sent to jcode exactly like typed input.

---

## 7. jcode Integration

The panel connects to jcode exclusively through its own client/server interface — no custom reimplementation of jcode features.

```
jcode server (background daemon)
       │
       ├── jcode CLI (terminal windows)     ← native, full TUI
       ├── jcode-panel (this app)           ← lightweight GUI client
       └── other jcode clients / agents     ← swarm, MCP, etc.
```

Everything jcode supports — multiagent swarm, memory, MCP tools, provider switching, skills — is available through the panel automatically because the panel is just a client.

The panel does **not** try to parse or interpret jcode's output — it streams it as-is into the chat view.

**Anything beyond a quick prompt? Open in terminal. That's the rule.**

---

## 8. Design System

### Color Palette (Dark, Minimal)

| Role | Color |
|---|---|
| Background (panel) | `#1e1e2e` |
| Background (header) | `#181825` |
| Surface / input | `#313244` |
| Border | `#45475a` |
| Text primary | `#cdd6f4` |
| Text secondary | `#6c7086` |
| Accent (interactive) | `#cba6f7` |
| Success | `#a6e3a1` |
| Error | `#f38ba8` |
| Info | `#89b4fa` |

### Typography
- UI text: System sans-serif, 12–13px
- Chat output: Monospace, 12px
- Input: Sans-serif, 13px

### Visual Style
- Rounded corners everywhere (`border-radius: 10–14px`)
- No heavy borders — only subtle `1px` dividers
- Soft drop shadows on the floating box
- Semi-transparent backgrounds where possible (floating box: ~92% opacity)
- No animations except smooth fade-in on popup open (~120ms)
- Icons: system icon theme or simple Unicode glyphs — no heavy icon libraries

---

## 9. Technical Stack

| Layer | Technology |
|---|---|
| Language | Python 3 |
| GUI framework | GTK3 (`gi.repository`) |
| Panel indicator | `AppIndicator3` |
| Global hotkey | `pynput` |
| Active window | `xdotool`, `xprop` |
| Browser context | Lightweight browser extension + local HTTP bridge |
| jcode connection | Formal jcode client API if stable; fallback to `jcode connect` subprocess streaming |
| Terminal launch | `subprocess` → configured terminal emulator |
| Voice (future) | `whisper.cpp` or system STT |
| Autostart | `.desktop` file in `~/.config/autostart/` |

---

## 10. File Structure

```
jcode-panel/
├── main.py              ← entry point, AppIndicator, hotkey listener
├── floating.py          ← floating input box (mouse-position popup)
├── dropdown.py          ← dropdown conversation panel
├── session_picker.py    ← resume session modal (opens native terminal)
├── jcode_client.py      ← jcode connect/stream/serve wrapper
├── context.py           ← active app + browser tab capture
├── voice.py             ← voice input stub (ready for later)
├── config.py            ← user settings (hotkey, terminal, theme)
├── assets/
│   └── icon.svg         ← top bar icon
├── extension/
│   ├── manifest.json    ← browser extension manifest
│   └── background.js    ← reports active tab URL to local bridge
└── install.sh           ← install deps, set up autostart, install extension
```

> `fullview.py` is removed — full view is handled by opening jcode natively in a terminal.

---

## 11. Configuration

A simple config file at `~/.config/jcode-panel/config.toml`:

```toml
[general]
hotkey = "f8"
terminal = "gnome-terminal"   # or kitty, alacritty, wezterm, etc.
autostart = true

[session]
auto_resume = true            # reconnect to last session on open
show_context_strip = true     # show app/tab context in floating box

[ui]
dropdown_max_messages = 20    # how many messages shown in dropdown
floating_opacity = 0.92       # semi-transparent floating box

[voice]
enabled = false               # stub — enable when STT is configured
hotkey = "alt+v"
```

---

## 12. Installation

```bash
# 1. Install system dependencies
sudo apt install python3-gi gir1.2-appindicator3-0.1 xdotool

# 2. Install Python dependencies
pip install pynput requests

# 3. Run the install script
cd jcode-panel
chmod +x install.sh
./install.sh

# 4. jcode-panel starts automatically on login
# Or launch manually:
python3 main.py
```

`install.sh` handles:
- Dependency checks
- Verifying jcode is installed and on `$PATH`
- Copying the browser extension to the correct location
- Creating `~/.config/autostart/jcode-panel.desktop`
- Running `jcode serve` as a background systemd user service (optional)

The browser extension is optional for v1. Without it, the panel still works using active application/window-title context. Browser URL, tab title, and selected text require the extension/local bridge.

---

## 13. Implementation Decisions

These decisions were resolved during implementation-readiness grilling and should be treated as part of the v1 contract.

### 13.1 jcode Client Transport

- The panel must be a full jcode client with access to slash commands, skills, spawning/subsessions, resume, memory, swarm, and MCP through jcode itself.
- Preferred transport: formal jcode server/client API if stable.
- Fallback transport: CLI subprocess streaming, e.g. `jcode connect` with stdin/stdout.
- Out of scope: passive log viewing as the main integration model.

### 13.2 Session Ownership and Persistence

- On first app start, create a new jcode client/session.
- Persist that session ID/name in config/state.
- On next app start, resume the saved session automatically.
- `+ New Session` switches the saved default to the newly created session.
- `↩ Resume` switches the saved default to the selected resumed session.
- Concurrent panel and terminal control of the same session is allowed.

### 13.3 Slash Commands and Completion

- `/commands` typed in the floating box execute through jcode, not through duplicated panel logic.
- The panel still provides native-like completion UX.
- Completion source priority:
  1. jcode formal completion API.
  2. jcode CLI completion fallback.
  3. Degraded no-completion mode if neither exists.

### 13.4 Rendering Strategy

Renderer priority:
1. Structured jcode event rendering, if exposed by jcode.
2. Basic markdown/plain text streaming.
3. Terminal emulation only as a last-resort compatibility fallback.

The panel should not reimplement jcode logic, but it may render structured jcode client events nicely.

### 13.5 Top Bar Preview

- Default behavior: smart status summary with meaningful progress and important errors, suppressing noisy logs.
- If `debug = true`, show the latest raw streamed text/message instead.
- The `✕` button hides the current preview and closes the dropdown if open. It does not quit the app.

### 13.6 Settings

- v1 includes both `~/.config/jcode-panel/config.toml` and a simple settings modal.
- The settings modal writes directly to `config.toml`.
- Common settings: hotkey, terminal template, debug mode, context default, dropdown message count, autostart.

### 13.7 Terminal Launching

- v1 supports auto-detected terminal adapters and user-editable command templates.
- Common targets include `gnome-terminal`, `kitty`, `alacritty`, and `wezterm`.

### 13.8 Missing or Broken jcode

- If `jcode` is missing, not logged in, or `jcode serve` fails, show clear error/instructions.
- Offer a button to open terminal for setup/login/retry.
- Do not install or configure jcode automatically unless the user explicitly runs an installer.

### 13.9 Platform Scope

- v1 is Ubuntu/GNOME X11-first.
- Detect Wayland and show a compatibility warning because hotkeys/window context may be limited.

### 13.10 MVP Acceptance Test

The smallest successful v1 demo:

1. Launch `jcode-panel`.
2. Press `F8`.
3. Floating input opens at cursor with context strip.
4. Type prompt and press Enter.
5. Prompt is sent to the saved jcode session.
6. Response streams in dropdown.
7. Quit/reopen panel resumes the same session.

---

## 14. Out of Scope (for now)

- Windows / macOS support (Ubuntu / GNOME only for v1)
- Built-in file browser, code editor, or diff viewer (use jcode natively in terminal)
- Custom AI provider configuration (use jcode's own `jcode login` flow)
- Inline terminal emulator as a primary/default UI (only acceptable as a last-resort compatibility fallback)
- Mobile companion app (jcode has iOS plans separately)

---

*This document reflects the design decisions made during discussion. Implementation follows this spec.*
