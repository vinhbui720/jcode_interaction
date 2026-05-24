#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
APP_DIR="$(pwd)"
BIN="$APP_DIR/src-tauri/target/release/jcode-panel-tauri"
USER_BIN="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
AUTOSTART_DIR="$HOME/.config/autostart"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
ICON_PNG_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"
GNOME_EXT_UUIDS=("jcode-cursor@local" "jcode-mouse@local" "jcode-focus@local")
GNOME_EXT_DBUS_SERVICES=("org.jcode.Panel.Cursor" "org.jcode.Panel.MouseHotkey" "org.jcode.Panel.Focus")
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

mkdir -p "$USER_BIN" "$DESKTOP_DIR" "$AUTOSTART_DIR" "$ICON_DIR" "$ICON_PNG_DIR"
for GNOME_EXT_UUID in "${GNOME_EXT_UUIDS[@]}"; do
  GNOME_EXT_SRC="$APP_DIR/gnome-extension/$GNOME_EXT_UUID"
  GNOME_EXT_DIR="$HOME/.local/share/gnome-shell/extensions/$GNOME_EXT_UUID"
  if [[ -d "$GNOME_EXT_SRC" ]]; then
    mkdir -p "$GNOME_EXT_DIR"
    cp "$GNOME_EXT_SRC/metadata.json" "$GNOME_EXT_SRC/extension.js" "$GNOME_EXT_DIR/"
    if command -v gnome-extensions >/dev/null 2>&1; then
      gnome-extensions enable "$GNOME_EXT_UUID" || true
    fi
    if command -v gsettings >/dev/null 2>&1; then
      python3 - "$GNOME_EXT_UUID" <<'PY'
import ast, subprocess, sys
uuid = sys.argv[1]
current = subprocess.run(["gsettings", "get", "org.gnome.shell", "enabled-extensions"], text=True, capture_output=True).stdout.strip()
try:
    values = ast.literal_eval(current.replace("@as ", ""))
except Exception:
    values = []
if uuid not in values:
    values.append(uuid)
subprocess.run(["gsettings", "set", "org.gnome.shell", "enabled-extensions", str(values)])
PY
    fi
  fi
done

wayland_extension_services_ready=true
if [[ -n "${WAYLAND_DISPLAY:-}" || "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
  for svc in "${GNOME_EXT_DBUS_SERVICES[@]}"; do
    if ! gdbus introspect --session --dest "$svc" --object-path "/${svc//./\/}" >/dev/null 2>&1; then
      wayland_extension_services_ready=false
      break
    fi
  done
fi
if [[ "$wayland_extension_services_ready" != true ]]; then
  cat <<'MSG'
Wayland integration note:
  Local GNOME extensions were installed, but their session D-Bus services are not active yet.
  GNOME Shell usually needs a logout/login cycle to load newly added local extensions.
  After logging back in, mouse tracking / cursor / prompt-focus integration should work.
MSG
fi
if [[ -f "$APP_DIR/../jcode-panel/assets/icon.svg" ]]; then
  cp "$APP_DIR/../jcode-panel/assets/icon.svg" "$ICON_DIR/jcode-panel.svg"
fi
if [[ -f "$APP_DIR/src-tauri/icons/icon.png" ]]; then
  cp "$APP_DIR/src-tauri/icons/icon.png" "$ICON_PNG_DIR/jcode-panel.png"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" || true
fi
cat > "$WRAPPER" <<EOF
#!/usr/bin/env bash
if [[ -z "\${JCODE_PANEL_GDK_BACKEND:-}" ]]; then
  if [[ -n "\${WAYLAND_DISPLAY:-}" || "\${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
    unset GDK_BACKEND
  elif [[ -n "\${DISPLAY:-}" ]]; then
    export GDK_BACKEND=x11
  fi
else
  export GDK_BACKEND="\$JCODE_PANEL_GDK_BACKEND"
fi
cd "$APP_DIR" || exit 1
exec "$BIN" "\$@"
EOF
chmod +x "$WRAPPER"
cat > "$USER_BIN/jcp" <<EOF
#!/usr/bin/env bash
if [[ -z "\${JCODE_PANEL_GDK_BACKEND:-}" ]]; then
  if [[ -n "\${WAYLAND_DISPLAY:-}" || "\${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
    unset GDK_BACKEND
  elif [[ -n "\${DISPLAY:-}" ]]; then
    export GDK_BACKEND=x11
  fi
else
  export GDK_BACKEND="\$JCODE_PANEL_GDK_BACKEND"
fi
cd "$APP_DIR" || exit 1
if [[ "\${1:-}" == "settings" || "\${1:-}" == "--settings" || "\${1:-}" == "dropdown" || "\${1:-}" == "--dropdown" || "\${1:-}" == "open" || "\${1:-}" == "--open" ]]; then
  exec "$BIN" dropdown
fi
if [[ "\${1:-}" == "prompt" || "\${1:-}" == "--prompt" || "\${1:-}" == "--show" ]]; then
  exec "$BIN" prompt
fi
if [[ "\$#" -eq 0 ]]; then
  exec "$BIN" dropdown
fi
exec "$BIN" "\$@"
EOF
chmod +x "$USER_BIN/jcp"

cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Jcode Interaction
Comment=Lightweight Rust/Tauri interaction client for jcode
Exec=$USER_BIN/jcp open
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
