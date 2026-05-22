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
    load_state_from_path(&state_path())
}

pub fn load_state_from_path(path: &PathBuf) -> AppState {
    let Ok(text) = fs::read_to_string(path) else {
        return AppState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_state(state: &AppState) -> anyhow::Result<()> {
    save_state_to_path(state, &state_path())
}

pub fn save_state_to_path(state: &AppState, path: &PathBuf) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_preserves_session_and_tokens() {
        let path =
            std::env::temp_dir().join(format!("jcode-panel-state-{}.json", std::process::id()));
        let state = AppState {
            active_session: Some("session-123".into()),
            active_section: "Work".into(),
            last_prompt: "hello".into(),
            token_stats: Some(TokenStats {
                upload: 1,
                download: 2,
                cache_read: 3,
                cache_write: 4,
            }),
            recent_messages: vec![ConversationMessage {
                author: "jcode".into(),
                text: "ok".into(),
            }],
        };
        save_state_to_path(&state, &path).unwrap();
        let loaded = load_state_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.active_session.as_deref(), Some("session-123"));
        assert_eq!(loaded.active_section, "Work");
        assert_eq!(loaded.token_stats.unwrap().download, 2);
        assert_eq!(loaded.recent_messages.len(), 1);
    }
}
