#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
APP_DIR="$(pwd)"
BIN="$APP_DIR/src-tauri/target/release/jcode-panel-tauri"
USER_BIN="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
AUTOSTART_DIR="$HOME/.config/autostart"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
DESKTOP_FILE="$DESKTOP_DIR/jcode-panel.desktop"
AUTOSTART_FILE="$AUTOSTART_DIR/jcode-panel.desktop"
WRAPPER="$USER_BIN/jcode-panel"

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
. "$HOME/.cargo/env"
rustup component add rustfmt clippy
npm install

npx tauri build --no-bundle

mkdir -p "$USER_BIN" "$DESKTOP_DIR" "$AUTOSTART_DIR" "$ICON_DIR"
if [[ -f "$APP_DIR/../jcode-panel/assets/icon.svg" ]]; then
  cp "$APP_DIR/../jcode-panel/assets/icon.svg" "$ICON_DIR/jcode-panel.svg"
elif [[ -f "$APP_DIR/src-tauri/icons/icon.png" ]]; then
  cp "$APP_DIR/src-tauri/icons/icon.png" "$ICON_DIR/jcode-panel.png"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" || true
fi
cat > "$WRAPPER" <<EOF
#!/usr/bin/env bash
export GDK_BACKEND=x11
cd "$APP_DIR" || exit 1
exec "$BIN" "\$@"
EOF
chmod +x "$WRAPPER"
cp "$WRAPPER" "$USER_BIN/jcp"
cat > "$USER_BIN/jcp" <<EOF
#!/usr/bin/env bash
export GDK_BACKEND=x11
cd "$APP_DIR" || exit 1
exec "$BIN" --prompt "\$@"
EOF
chmod +x "$USER_BIN/jcp"

cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Jcode Interaction
Comment=Lightweight Rust/Tauri interaction client for jcode
Exec=$WRAPPER
Icon=jcode-panel
Terminal=false
Categories=Utility;Development;
StartupNotify=true
EOF

cat > "$AUTOSTART_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Jcode Interaction
Comment=Lightweight Rust/Tauri interaction client for jcode
Exec=$WRAPPER
Icon=jcode-panel
Terminal=false
Categories=Utility;Development;
StartupNotify=true
X-GNOME-Autostart-enabled=true
EOF

# Stop legacy Python panel if it was left by an older autostart entry.
python3 - <<'PY'
import os
import signal
import subprocess

patterns = ["python3 -m jcode_panel.main"]
current = {os.getpid(), os.getppid()}
rows = subprocess.run(["ps", "-eo", "pid=,args="], text=True, capture_output=True).stdout.splitlines()
for row in rows:
    row = row.strip()
    if not row:
        continue
    pid_text, _, args = row.partition(" ")
    try:
        pid = int(pid_text)
    except ValueError:
        continue
    if pid in current:
        continue
    if any(pattern in args for pattern in patterns):
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
PY

cat <<MSG
Tauri app is built and installed as the active jcode-panel runtime.

Launchers:
  $WRAPPER
  $USER_BIN/jcp
  $DESKTOP_FILE
  $AUTOSTART_FILE

Run now:
  jcode-panel
MSG
