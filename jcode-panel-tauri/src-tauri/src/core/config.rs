use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub prompt_hotkey: String,
    pub screenshot_hotkey: String,
    pub terminal: String,
    pub send_context_default: bool,
    pub max_prompt_chars: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            prompt_hotkey: "F8".into(),
            screenshot_hotkey: "Ctrl+Shift+S".into(),
            terminal: "gnome-terminal".into(),
            send_context_default: true,
            max_prompt_chars: 4000,
        }
    }
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
    toml::from_str(&text)
        .or_else(|_| migrate_legacy_config(&text))
        .unwrap_or_default()
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
        };
        save_config_to_path(&config, &path).unwrap();
        let loaded = load_config_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.prompt_hotkey, "F9");
        assert_eq!(loaded.terminal, "wezterm");
        assert!(!loaded.send_context_default);
        assert_eq!(loaded.max_prompt_chars, 1234);
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
    }
}

#[cfg(test)]
fn load_config_from_text(text: &str) -> AppConfig {
    toml::from_str(text)
        .or_else(|_| migrate_legacy_config(text))
        .unwrap_or_default()
}
