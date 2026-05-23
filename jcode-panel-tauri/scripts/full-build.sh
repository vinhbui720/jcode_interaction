#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
APP_DIR="$(pwd)"
BIN="$APP_DIR/src-tauri/target/release/jcode-panel-tauri"
USER_BIN="$HOME/.local/bin"
RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
RESTART=0

for arg in "$@"; do
  case "$arg" in
    --restart) RESTART=1 ;;
    --no-restart) RESTART=0 ;;
    *) echo "Unknown argument: $arg" >&2; exit 2 ;;
  esac
done

printf '\n==> Type check frontend\n'
npm run check

printf '\n==> Build frontend assets\n'
npm run build:web

printf '\n==> Format Rust\n'
cargo fmt --manifest-path src-tauri/Cargo.toml

printf '\n==> Test Rust\n'
cargo test --manifest-path src-tauri/Cargo.toml

printf '\n==> Build Tauri release binary\n'
npx tauri build --no-bundle

printf '\n==> Install local launchers\n'
mkdir -p "$USER_BIN"
cat > "$USER_BIN/jcode-panel" <<EOF
#!/usr/bin/env bash
export GDK_BACKEND=x11
cd "$APP_DIR" || exit 1
exec "$BIN" "\$@"
EOF
chmod +x "$USER_BIN/jcode-panel"
cat > "$USER_BIN/jcp" <<EOF
#!/usr/bin/env bash
export GDK_BACKEND=x11
cd "$APP_DIR" || exit 1
case "\${1:-}" in
  settings|--settings|dropdown|--dropdown|open|--open) exec "$BIN" dropdown ;;
  prompt|--prompt|--show) exec "$BIN" prompt ;;
  "") exec "$BIN" dropdown ;;
  *) exec "$BIN" "\$@" ;;
esac
EOF
chmod +x "$USER_BIN/jcp"

if [[ "$RESTART" == "1" ]]; then
  printf '\n==> Stop stale app processes\n'
  python3 - <<'PY'
import os, signal, time
suffix = '/jcode-panel-tauri/src-tauri/target/release/jcode-panel-tauri'
pids = []
for pid_s in os.listdir('/proc'):
    if not pid_s.isdigit():
        continue
    pid = int(pid_s)
    try:
        exe = os.readlink(f'/proc/{pid_s}/exe').removesuffix(' (deleted)')
    except Exception:
        continue
    if exe.endswith(suffix):
        pids.append(pid)
for pid in pids:
    print(f'term {pid}')
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
time.sleep(0.8)
for pid in pids:
    if os.path.exists(f'/proc/{pid}'):
        print(f'kill {pid}')
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
PY
  rm -f "$RUNTIME_DIR/jcode-panel-tauri.pid" "$RUNTIME_DIR/jcode-panel-tauri.sock"
  printf '\n==> Start fresh dropdown\n'
  nohup "$USER_BIN/jcp" dropdown >/tmp/jcode-panel-current.log 2>&1 &
  sleep 1
  pgrep -af 'jcode-panel-tauri.*dropdown' || true
fi

printf '\nDone. Release binary: %s\n' "$BIN"
