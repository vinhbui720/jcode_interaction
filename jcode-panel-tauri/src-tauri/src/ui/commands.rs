use crate::{
    core::{config, jcode, state},
    integrations,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

pub struct RuntimeState(pub Mutex<state::AppState>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub config: config::AppConfig,
    pub state: state::AppState,
    pub jcode_available: bool,
}

#[tauri::command]
pub fn snapshot(runtime: State<RuntimeState>) -> AppSnapshot {
    AppSnapshot {
        config: config::load_config(),
        state: runtime.0.lock().expect("state lock").clone(),
        jcode_available: jcode::jcode_available(),
    }
}

#[tauri::command]
pub fn save_settings(new_config: config::AppConfig) -> Result<(), String> {
    config::save_config(&new_config).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn submit_prompt(
    prompt: String,
    runtime: State<RuntimeState>,
) -> Result<jcode::SendResult, String> {
    let session = runtime.0.lock().expect("state lock").active_session.clone();
    let result = jcode::send_prompt(&prompt, session.as_deref()).map_err(|err| err.to_string())?;
    let mut state = runtime.0.lock().expect("state lock");
    state.last_prompt = prompt;
    state.recent_messages.push(state::ConversationMessage {
        author: "You".into(),
        text: state.last_prompt.clone(),
    });
    state.recent_messages.push(state::ConversationMessage {
        author: "jcode".into(),
        text: result.output.clone(),
    });
    if let Some(session_id) = &result.session_id {
        state.active_session = Some(session_id.clone());
    }
    state::save_state(&state).map_err(|err| err.to_string())?;
    Ok(result)
}

#[tauri::command]
pub fn integration_status() -> serde_json::Value {
    serde_json::json!({
        "vscode": integrations::vscode::status(),
        "obsidian": integrations::obsidian::status()
    })
}

#[tauri::command]
pub fn refresh_integrations() -> serde_json::Value {
    serde_json::json!({
        "vscode": integrations::vscode::install(),
        "obsidian": integrations::obsidian::install()
    })
}
