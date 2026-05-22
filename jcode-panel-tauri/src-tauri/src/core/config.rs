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
    let path = config_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, toml::to_string_pretty(config)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
