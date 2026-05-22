# jcode-panel-tauri

Rust/Tauri replacement for the Python `jcode-panel` app.

This is a real desktop app, not a browser tab. Tauri uses native desktop windows with the system webview, so the app keeps the resident tray/header icon, floating prompt popup, feedback toast, settings, and dropdown behavior.

## Current app behavior

- Resident tray/header icon with startup notification.
- `F8` opens the floating prompt popup repeatedly.
- Prompt follows the mouse and uses the compact `[JI] input count` layout.
- Smart prompt suggestions:
  - `@` suggests app context targets like `@vscode` and `@obsidian`.
  - `/` suggests panel/jcode commands.
  - `Tab` completes the selected suggestion.
- Feedback toast replays current feedback, has transparent styling, scrollable/selectable text, bottom status bar, and token upload/download/cache badges.
- Settings, token state, prompt history, active session/resume state, and browser bridge state persist across restart/boot.
- VS Code and Obsidian integration install/refresh is included.

## Install the current Rust/Tauri app

From this repo root:

```bash
git checkout rust/tauri
git pull --ff-only origin rust/tauri
cd jcode-panel-tauri
./setup.sh
```

`setup.sh` is the one-command installer/update path. It will:

1. Install Rust with `rustup` if `cargo` is missing.
2. Install Rust components used by this repo: `rustfmt` and `clippy`.
3. Run `npm install`.
4. Build the release Tauri app with `npm run build`.
5. Install launch wrappers:
   - `~/.local/bin/jcode-panel`
   - `~/.local/bin/jcp`
6. Install desktop entries:
   - `~/.local/share/applications/jcode-panel.desktop`
   - `~/.config/autostart/jcode-panel.desktop`
7. Install the themed icon:
   - `~/.local/share/icons/hicolor/scalable/apps/jcode-panel.svg`
8. Stop stale legacy Python panel processes from old autostart entries.

After install, launch with:

```bash
jcode-panel
# or
jcp
```

## Linux system dependencies

If the Tauri build fails because native Linux libraries are missing, install these first:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  curl \
  pkg-config \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  xdotool \
  gnome-screenshot
```

Then rerun:

```bash
cd jcode-panel-tauri
./setup.sh
```

## Update an existing install

```bash
cd /path/to/jcode_interaction
git checkout rust/tauri
git pull --ff-only origin rust/tauri
cd jcode-panel-tauri
./setup.sh
```

Restart the active app after update:

```bash
pkill -f '/jcode-panel-tauri/src-tauri/target/release/jcode-panel-tauri' || true
rm -f /tmp/jcode-panel-tauri.pid
jcode-panel &
```

## Verify the Rust app is active

```bash
ps -eo pid=,comm=,args= | grep -E 'jcode-panel-tauri|jcode_panel' | grep -v grep
```

Expected:

- One `jcode-panel-tauri` process.
- No Python `jcode_panel` process.

Quick count check:

```bash
python3 - <<'PY'
import subprocess
rows=subprocess.check_output(['ps','-eo','pid=,comm=,args='], text=True).splitlines()
rust=[]; py=[]
for row in rows:
    parts=row.strip().split(None,2)
    if len(parts)<3: continue
    pid, comm, cmd=parts
    if cmd.endswith('/jcode-panel-tauri/src-tauri/target/release/jcode-panel-tauri'):
        rust.append(pid)
    if comm.startswith('python') and ('jcode_panel.gtk_app' in cmd or '/jcode_panel/gtk_app.py' in cmd or 'jcode_panel.main' in cmd):
        py.append(pid)
print(f'python_count={len(py)} rust_count={len(rust)}')
PY
```

Expected output:

```text
python_count=0 rust_count=1
```

## Development commands

```bash
# Web/type checks
npm run check
npm run build:web

# Rust tests
cd src-tauri
cargo test -- --test-threads=1

# Full release build
cd ..
npm run build
```

## Git push workflow for this branch

After code or docs changes:

```bash
git status -sb
git add <changed-files>
git commit -m "Your concise commit message"
git push origin rust/tauri
```

Before pushing code changes, run at least:

```bash
cd jcode-panel-tauri
npm run check
npm run build:web
cd src-tauri
cargo test -- --test-threads=1
```

For release/runtime changes, also run:

```bash
cd jcode-panel-tauri
npm run build
```
