use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub prompt_hotkey: String,
    pub screenshot_hotkey: String,
    pub terminal: String,
    pub send_context_default: bool,
    pub max_prompt_chars: usize,
    pub tts_enabled: bool,
    pub tts_api_url: String,
    pub tts_command: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            prompt_hotkey: "F8".into(),
            screenshot_hotkey: "Ctrl+Shift+S".into(),
            terminal: "gnome-terminal".into(),
            send_context_default: true,
            max_prompt_chars: 4000,
            tts_enabled: false,
            tts_api_url: String::new(),
            tts_command: default_tts_command(),
        }
    }
}

fn default_tts_command() -> String {
    "SUPER=${JCODE_SUPERTONIC_DIR:-$PWD/../supertonic}; cd \"$SUPER/py\" && mkdir -p /tmp/jcode-supertonic-tts && rm -f /tmp/jcode-supertonic-tts/*.wav && uv run python example_onnx.py --onnx-dir ../assets/onnx --voice-style ../assets/voice_styles/M1.json --text {text} --lang en --n-test 1 --save-dir /tmp/jcode-supertonic-tts >/tmp/jcode-supertonic-tts/last.log 2>&1 && wav=$(ls -t /tmp/jcode-supertonic-tts/*.wav 2>/dev/null | head -n1) && { command -v paplay >/dev/null && paplay \"$wav\" || aplay \"$wav\"; }".into()
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("jcode-panel")
        .join("config.toml")
}

pub fn load_config() -> AppConfig {
    load_config_from_path(&config_path())
}

pub fn load_config_from_path(path: &PathBuf) -> AppConfig {
    let Ok(text) = fs::read_to_string(path) else {
        return AppConfig::default();
    };
    load_config_from_text(&text)
}

fn load_config_from_text(text: &str) -> AppConfig {
    if text.contains("[general]") || text.contains("[session]") {
        return migrate_legacy_config(text).unwrap_or_default();
    }
    toml::from_str(text).unwrap_or_default()
}

fn migrate_legacy_config(text: &str) -> Result<AppConfig, toml::de::Error> {
    #[derive(Deserialize)]
    struct LegacyConfig {
        general: Option<LegacyGeneral>,
        session: Option<LegacySession>,
    }
    #[derive(Deserialize)]
    struct LegacyGeneral {
        hotkey: Option<String>,
        screenshot_hotkey: Option<String>,
        terminal: Option<String>,
    }
    #[derive(Deserialize)]
    struct LegacySession {
        send_context_default: Option<bool>,
    }

    let legacy: LegacyConfig = toml::from_str(text)?;
    let defaults = AppConfig::default();
    Ok(AppConfig {
        prompt_hotkey: legacy
            .general
            .as_ref()
            .and_then(|general| general.hotkey.clone())
            .unwrap_or(defaults.prompt_hotkey),
        screenshot_hotkey: legacy
            .general
            .as_ref()
            .and_then(|general| general.screenshot_hotkey.clone())
            .unwrap_or(defaults.screenshot_hotkey),
        terminal: legacy
            .general
            .and_then(|general| general.terminal)
            .unwrap_or(defaults.terminal),
        send_context_default: legacy
            .session
            .and_then(|session| session.send_context_default)
            .unwrap_or(defaults.send_context_default),
        max_prompt_chars: defaults.max_prompt_chars,
        tts_enabled: defaults.tts_enabled,
        tts_api_url: defaults.tts_api_url,
        tts_command: defaults.tts_command,
    })
}

pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    save_config_to_path(config, &config_path())
}

pub fn save_config_to_path(config: &AppConfig, path: &PathBuf) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, toml::to_string_pretty(config)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip_preserves_settings() {
        let path =
            std::env::temp_dir().join(format!("jcode-panel-config-{}.toml", std::process::id()));
        let config = AppConfig {
            prompt_hotkey: "F9".into(),
            screenshot_hotkey: "Ctrl+Alt+S".into(),
            terminal: "wezterm".into(),
            send_context_default: false,
            max_prompt_chars: 1234,
            tts_enabled: true,
            tts_api_url: "http://localhost:9876/tts".into(),
            tts_command: "echo {text}".into(),
        };
        save_config_to_path(&config, &path).unwrap();
        let loaded = load_config_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.prompt_hotkey, "F9");
        assert_eq!(loaded.terminal, "wezterm");
        assert!(!loaded.send_context_default);
        assert_eq!(loaded.max_prompt_chars, 1234);
        assert!(loaded.tts_enabled);
        assert_eq!(loaded.tts_api_url, "http://localhost:9876/tts");
        assert_eq!(loaded.tts_command, "echo {text}");
    }

    #[test]
    fn legacy_config_migrates_without_falling_back_to_defaults() {
        let config = load_config_from_text(
            r##"
[general]
hotkey = "super+z"
screenshot_hotkey = "super+x"
terminal = "wezterm"

[session]
send_context_default = false
"##,
        );
        assert_eq!(config.prompt_hotkey, "super+z");
        assert_eq!(config.screenshot_hotkey, "super+x");
        assert_eq!(config.terminal, "wezterm");
        assert!(!config.send_context_default);
        assert!(!config.tts_enabled);
        assert!(config.tts_command.contains("supertonic"));
    }

    #[test]
    fn missing_new_tts_fields_load_from_defaults() {
        let config = load_config_from_text(
            r##"
prompt_hotkey = "F8"
screenshot_hotkey = "Ctrl+Shift+S"
terminal = "wezterm"
send_context_default = true
max_prompt_chars = 4000
"##,
        );
        assert!(!config.tts_enabled);
        assert!(config.tts_command.contains("example_onnx.py"));
    }
}
