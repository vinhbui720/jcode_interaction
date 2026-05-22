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

python_missing=0
PYTHONPATH="$PWD:$PWD/.python-deps" python3 - <<'PY' || python_missing=1
import importlib.util
missing = []
for mod in ["requests", "pynput"]:
    if importlib.util.find_spec(mod) is None:
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

if [[ "$python_missing" -ne 0 ]]; then
  missing=1
fi
deps_missing="$missing"

mkdir -p "$HOME/.config/autostart" "$HOME/.local/bin" "$HOME/.local/share/icons/hicolor/scalable/apps"
cat > "$HOME/.config/autostart/jcode-panel.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Jcode Interaction
Exec=env GDK_BACKEND=x11 PYTHONPATH=$PWD:$PWD/.python-deps python3 -m jcode_panel.main --background
X-GNOME-Autostart-enabled=true
EOF
mkdir -p "$HOME/.local/share/applications"
cp assets/icon.svg "$HOME/.local/share/icons/hicolor/scalable/apps/jcode-panel.svg"
ln -sf "$PWD/bin/jcode-panel" "$HOME/.local/bin/jcode-panel"
ln -sf "$PWD/bin/jcp" "$HOME/.local/bin/jcp"
sed "s#Exec=jcode-panel#Exec=$HOME/.local/bin/jcode-panel#; s#Icon=applications-system#Icon=jcode-panel#" jcode-panel.desktop > "$HOME/.local/share/applications/jcode-panel.desktop"
# Older installs exposed a second app-grid launcher for the prompt. The prompt
# remains available through F8, tray menu, and `jcp`, but only one visible app is
# installed: Jcode Interaction.
rm -f "$HOME/.local/share/applications/jcode-panel-prompt.desktop"

PYTHONPATH="$PWD:$PWD/.python-deps" python3 -m jcode_panel.main --diagnose || true
PYTHONPATH="$PWD:$PWD/.python-deps" python3 -m jcode_panel.main --install-shortcut || true

echo "Checking app integrations..."
for integration in vscode obsidian; do
  if PYTHONPATH="$PWD:$PWD/.python-deps" python3 -m jcode_panel.main --install-integration "$integration"; then
    echo "OK: $integration integration installed/refreshed"
  else
    echo "SKIP: $integration integration not available on this machine"
  fi
done

echo "Installed autostart entry. VS Code and Obsidian integrations are auto-refreshed when detectable. Browser extension is in ./extension and is optional."
echo "Aliases installed:"
echo "  jcode-panel        # open app"
echo "  jcp                # open prompt"
echo "Desktop launcher installed as Jcode Interaction."

if [[ "$deps_missing" -ne 0 ]]; then
  cat >&2 <<'EOF'

Launchers were installed, but runtime dependencies are missing.
Install suggested Ubuntu dependencies:
  sudo apt install python3-gi gir1.2-appindicator3-0.1 gir1.2-gtk-3.0 xdotool x11-utils
  python3 -m pip install --user pynput requests
EOF
  exit 1
fi
