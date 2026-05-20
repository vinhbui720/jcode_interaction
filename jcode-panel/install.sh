#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v jcode >/dev/null 2>&1; then
  echo "ERROR: jcode is not on PATH. Install/login to jcode first." >&2
  exit 1
fi

python3 - <<'PY'
try:
    import gi
    gi.require_version('Gtk', '3.0')
    gi.require_version('AppIndicator3', '0.1')
except Exception as exc:
    raise SystemExit(f"Missing GTK/AppIndicator deps: {exc}")
PY

mkdir -p "$HOME/.config/autostart" "$HOME/.local/bin"
cat > "$HOME/.config/autostart/jcode-panel.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=jcode-panel
Exec=python3 -m jcode_panel.main
X-GNOME-Autostart-enabled=true
EOF

echo "Installed autostart entry. Browser extension is in ./extension and is optional."
