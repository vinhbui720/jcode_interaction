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
    let user_prompt = prompt.clone();
    state.last_prompt = prompt;
    state.remember_prompt(&user_prompt);
    state.recent_messages.push(state::ConversationMessage {
        author: "You".into(),
        text: user_prompt,
    });
    state.recent_messages.push(state::ConversationMessage {
        author: "jcode".into(),
        text: result.output.clone(),
    });
    if let Some(session_id) = &result.session_id {
        state.active_session = Some(session_id.clone());
    }
    if let Some(token_stats) = &result.token_stats {
        state.token_stats = Some(token_stats.clone());
    }
    state::save_state(&state).map_err(|err| err.to_string())?;
    Ok(result)
}

#[tauri::command]
pub fn switch_session(
    session: String,
    name: Option<String>,
    runtime: State<RuntimeState>,
) -> Result<state::AppState, String> {
    let mut state = runtime.0.lock().expect("state lock");
    state.active_session = Some(session.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(name) = name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
    {
        state.active_section = name;
    }
    state::save_state(&state).map_err(|err| err.to_string())?;
    Ok(state.clone())
}

#[tauri::command]
pub fn start_new_section(
    name: Option<String>,
    runtime: State<RuntimeState>,
) -> Result<state::AppState, String> {
    let mut state = runtime.0.lock().expect("state lock");
    state.active_session = None;
    state.active_section = name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Fresh Panel".into());
    state.recent_messages.clear();
    state::save_state_to_path_preserving(&state, &state::state_path(), true)
        .map_err(|err| err.to_string())?;
    Ok(state.clone())
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
