use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenStats {
    pub upload: u64,
    pub download: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub active_session: Option<String>,
    pub active_section: String,
    pub last_prompt: String,
    pub token_stats: Option<TokenStats>,
    pub recent_messages: Vec<ConversationMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub author: String,
    pub text: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_session: None,
            active_section: "Fresh Panel".into(),
            last_prompt: String::new(),
            token_stats: None,
            recent_messages: vec![],
        }
    }
}

pub fn state_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("jcode-panel")
        .join("state.json")
}

pub fn load_state() -> AppState {
    let path = state_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return AppState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_state(state: &AppState) -> anyhow::Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
