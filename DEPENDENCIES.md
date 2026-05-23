# External dependencies

This repository intentionally keeps large/fast-moving third-party projects out of git. Clone them beside this repo when a feature needs them.

## Required build/runtime tools

Install these with the OS package manager before building the Tauri app on Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential curl pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  xdotool xclip xsel gnome-screenshot \
  pulseaudio-utils alsa-utils
```

Other required toolchains:

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
rustup component add rustfmt clippy

# Node dependencies
cd jcode-panel-tauri
npm install
```

## Optional VCS dependency: Supertonic local TTS

The feedback sound feature can use either an HTTP TTS API or a shell command. The default command expects Supertonic to be cloned as a sibling directory of `jcode-panel-tauri`:

```text
jcode_interaction/
  jcode-panel-tauri/
  supertonic/
```

Clone with VCS:

```bash
cd /path/to/jcode_interaction
git clone https://github.com/supertone-inc/supertonic.git supertonic
cd supertonic
git checkout main
cd py
uv sync || uv python install || true
```

Verified local revision during development:

```text
repo: https://github.com/supertone-inc/supertonic.git
branch: main
revision: dff55dc
```

Smoke test:

```bash
cd /path/to/jcode_interaction/supertonic/py
uv run python example_onnx.py \
  --onnx-dir ../assets/onnx \
  --voice-style ../assets/voice_styles/M1.json \
  --text "Hello" \
  --lang en \
  --n-test 1
```

If Supertonic is somewhere else, set a custom command in the app settings or export `JCODE_SUPERTONIC_DIR` before launching `jcode-panel`.

## Fresh machine install from VCS

```bash
git clone git@github.com:vinhbui720/jcode_interaction.git
cd jcode_interaction
git checkout rust/tauri

# Optional TTS backend
git clone https://github.com/supertone-inc/supertonic.git supertonic

cd jcode-panel-tauri
./setup.sh
jcode-panel &
```

## Browser integration

The browser bridge files are tracked under `jcode-panel/extension/`. After updating browser selection metadata code, reload the extension in the browser so `selectionchange` reporting is active.
