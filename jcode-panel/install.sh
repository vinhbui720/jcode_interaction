#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

missing=0
need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "MISSING: $1" >&2
    missing=1
  else
    echo "OK: $1 -> $(command -v "$1")"
  fi
}

need_cmd python3
need_cmd jcode
need_cmd xdotool
need_cmd xprop

python3 - <<'PY'
missing = []
for mod in ["requests", "pynput"]:
    try:
        __import__(mod)
    except Exception:
        missing.append(mod)
try:
    import gi
    gi.require_version('Gtk', '3.0')
    gi.require_version('AppIndicator3', '0.1')
except Exception as exc:
    missing.append(f"GTK/AppIndicator ({exc})")
if missing:
    raise SystemExit("Missing Python/system modules: " + ", ".join(missing))
print("OK: Python dependencies")
PY

if [[ "$missing" -ne 0 ]]; then
  cat >&2 <<'EOF'
Install suggested Ubuntu dependencies:
  sudo apt install python3-gi gir1.2-appindicator3-0.1 gir1.2-gtk-3.0 xdotool x11-utils
  python3 -m pip install --user pynput requests
EOF
  exit 1
fi

mkdir -p "$HOME/.config/autostart" "$HOME/.local/bin" "$HOME/.local/share/icons/hicolor/scalable/apps"
cat > "$HOME/.config/autostart/jcode-panel.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=jcode-panel
Exec=env PYTHONPATH=$PWD python3 -m jcode_panel.main
X-GNOME-Autostart-enabled=true
EOF
mkdir -p "$HOME/.local/share/applications"
cp assets/icon.svg "$HOME/.local/share/icons/hicolor/scalable/apps/jcode-panel.svg"
ln -sf "$PWD/bin/jcode-panel" "$HOME/.local/bin/jcode-panel"
ln -sf "$PWD/bin/jcp" "$HOME/.local/bin/jcp"
sed "s#Exec=jcode-panel#Exec=$HOME/.local/bin/jcode-panel#; s#Icon=applications-system#Icon=jcode-panel#" jcode-panel.desktop > "$HOME/.local/share/applications/jcode-panel.desktop"
sed "s#Exec=jcode-panel --prompt#Exec=$HOME/.local/bin/jcp#; s#Icon=applications-system#Icon=jcode-panel#" jcode-panel-prompt.desktop > "$HOME/.local/share/applications/jcode-panel-prompt.desktop"

PYTHONPATH="$PWD" python3 -m jcode_panel.main --diagnose || true

echo "Installed autostart entry. Browser extension is in ./extension and is optional."
echo "Aliases installed:"
echo "  jcode-panel        # open app"
echo "  jcp                # open prompt"
echo "Desktop launchers installed with jcode-panel icon."
