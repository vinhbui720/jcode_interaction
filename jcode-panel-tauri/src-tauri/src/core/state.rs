use crate::core::activity::{self, LiveActivity};
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
    #[serde(default)]
    pub active_session: Option<String>,
    #[serde(default = "default_active_section")]
    pub active_section: String,
    #[serde(default)]
    pub last_prompt: String,
    #[serde(default)]
    pub token_stats: Option<TokenStats>,
    #[serde(default)]
    pub recent_messages: Vec<ConversationMessage>,
    #[serde(default)]
    pub prompt_history: Vec<String>,
    #[serde(default)]
    pub last_context_summary: String,
    #[serde(default)]
    pub browser_bridge_seen: bool,
    #[serde(default = "default_process_status")]
    pub process_status: String,
    #[serde(default)]
    pub live_activity: Option<LiveActivity>,
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
            prompt_history: vec![],
            last_context_summary: String::new(),
            browser_bridge_seen: false,
            process_status: default_process_status(),
            live_activity: None,
        }
    }
}

fn default_active_section() -> String {
    "Fresh Panel".into()
}

fn default_process_status() -> String {
    "idle".into()
}

impl AppState {
    pub fn ready_client_name(&self) -> String {
        let section = self.active_section.trim();
        if !section.is_empty() && section != "Fresh Panel" {
            section.to_string()
        } else {
            "jcode".into()
        }
    }

    pub fn remember_prompt(&mut self, prompt: &str) {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return;
        }
        self.prompt_history.retain(|item| item != prompt);
        self.prompt_history.push(prompt.to_string());
        let overflow = self.prompt_history.len().saturating_sub(100);
        if overflow > 0 {
            self.prompt_history.drain(0..overflow);
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
    let state = normalize_runtime_state(load_state_from_path(&path));
    let _ = save_state_to_path_preserving(&state, &path, true);
    state
}

fn normalize_runtime_state(mut state: AppState) -> AppState {
    if matches!(state.process_status.as_str(), "idle" | "complete" | "error") {
        state.process_status = activity::IDLE_STATUS.into();
        state.live_activity = None;
    }
    state
}

pub fn load_state_from_path(path: &PathBuf) -> AppState {
    let Ok(text) = fs::read_to_string(path) else {
        return AppState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_state(state: &AppState) -> anyhow::Result<()> {
    save_state_to_path_preserving(state, &state_path(), false)
}

pub fn save_state_to_path(state: &AppState, path: &PathBuf) -> anyhow::Result<()> {
    save_state_to_path_preserving(state, path, true)
}

pub fn save_state_to_path_preserving(
    state: &AppState,
    path: &PathBuf,
    allow_clear_session: bool,
) -> anyhow::Result<()> {
    let mut state = state.clone();
    if !allow_clear_session {
        let existing = load_state_from_path(path);
        if state.active_session.is_none() && existing.active_session.is_some() {
            state.active_session = existing.active_session;
        }
        if state.token_stats.is_none() && existing.token_stats.is_some() {
            state.token_stats = existing.token_stats;
        }
        if state.recent_messages.is_empty() && !existing.recent_messages.is_empty() {
            state.recent_messages = existing.recent_messages;
        }
        if state.prompt_history.is_empty() && !existing.prompt_history.is_empty() {
            state.prompt_history = existing.prompt_history;
        }
        if state.last_context_summary.is_empty() && !existing.last_context_summary.is_empty() {
            state.last_context_summary = existing.last_context_summary;
        }
        if !state.browser_bridge_seen && existing.browser_bridge_seen {
            state.browser_bridge_seen = true;
        }
        if state.process_status.is_empty() && !existing.process_status.is_empty() {
            state.process_status = existing.process_status;
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&state)?)?;
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
            ..AppState::default()
        };
        save_state_to_path(&state, &path).unwrap();
        let loaded = load_state_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.active_session.as_deref(), Some("session-123"));
        assert_eq!(loaded.active_section, "Work");
        assert_eq!(loaded.token_stats.unwrap().download, 2);
        assert_eq!(loaded.recent_messages.len(), 1);
    }

    #[test]
    fn state_save_preserves_existing_session_and_tokens_when_blank() {
        let path = std::env::temp_dir().join(format!(
            "jcode-panel-state-preserve-{}.json",
            std::process::id()
        ));
        let existing = AppState {
            active_session: Some("keep-me".into()),
            token_stats: Some(TokenStats {
                upload: 9,
                download: 8,
                cache_read: 7,
                cache_write: 6,
            }),
            ..AppState::default()
        };
        save_state_to_path(&existing, &path).unwrap();
        save_state_to_path_preserving(&AppState::default(), &path, false).unwrap();
        let loaded = load_state_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.active_session.as_deref(), Some("keep-me"));
        assert_eq!(loaded.token_stats.unwrap().upload, 9);
    }

    #[test]
    fn state_save_does_not_resurrect_cleared_live_activity() {
        let path = std::env::temp_dir().join(format!(
            "jcode-panel-state-live-{}.json",
            std::process::id()
        ));
        let existing = AppState {
            process_status: "sending".into(),
            live_activity: Some(LiveActivity::new("jcode", "sending")),
            ..AppState::default()
        };
        save_state_to_path(&existing, &path).unwrap();
        let complete = AppState {
            process_status: "complete".into(),
            live_activity: None,
            ..AppState::default()
        };
        save_state_to_path_preserving(&complete, &path, false).unwrap();
        let loaded = load_state_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.process_status, "complete");
        assert!(loaded.live_activity.is_none());
    }

    #[test]
    fn state_save_can_intentionally_clear_session_for_new_section() {
        let path = std::env::temp_dir().join(format!(
            "jcode-panel-state-clear-{}.json",
            std::process::id()
        ));
        let existing = AppState {
            active_session: Some("clear-me".into()),
            ..AppState::default()
        };
        save_state_to_path(&existing, &path).unwrap();
        save_state_to_path_preserving(&AppState::default(), &path, true).unwrap();
        let loaded = load_state_from_path(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.active_session, None);
    }
}
