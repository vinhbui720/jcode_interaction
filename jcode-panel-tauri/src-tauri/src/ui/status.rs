use crate::{
    core::{activity, state},
    ui::commands::RuntimeState,
};
use tauri::{AppHandle, Manager};

pub fn set_process_status(app: &AppHandle, process_status: &str) -> Result<(), String> {
    let status = process_status.trim();
    let status = if status.is_empty() {
        activity::IDLE_STATUS
    } else {
        status
    };
    {
        let runtime = app.state::<RuntimeState>();
        let mut state = runtime.0.lock().expect("state lock");
        state.process_status = status.to_string();
        state.live_activity = None;
        state::save_state(&state).map_err(|err| err.to_string())?;
    }
    set_header_status(app, status)
}

pub fn start_activity(app: &AppHandle, process_status: &str, label: &str) -> Result<(), String> {
    let activity_state = process_status.trim();
    let activity_state = if activity_state.is_empty() {
        activity::SENDING_STATUS
    } else {
        activity_state
    };
    let live_activity = activity::LiveActivity::new(label, activity_state);
    let header = activity::header_status(activity_state, Some(&live_activity));
    {
        let runtime = app.state::<RuntimeState>();
        let mut state = runtime.0.lock().expect("state lock");
        state.process_status = activity_state.to_string();
        state.live_activity = Some(live_activity);
        state::save_state(&state).map_err(|err| err.to_string())?;
    }
    set_header_status(app, &header)
}

pub fn refresh_header_status(app: &AppHandle) -> Result<(), String> {
    let state = app
        .state::<RuntimeState>()
        .0
        .lock()
        .expect("state lock")
        .clone();
    let header = activity::header_status(&state.process_status, state.live_activity.as_ref());
    set_header_status(app, &header)
}

pub fn set_header_status(app: &AppHandle, label: &str) -> Result<(), String> {
    let label = activity::truncate_header_label(label.trim());
    if let Some(tray) = app.tray_by_id("jcode-panel") {
        tray.set_title(Some(&label))
            .map_err(|err| err.to_string())?;
        tray.set_tooltip(Some(&format!("Jcode Interaction · {label}")))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn record_user_prompt(app: &AppHandle, prompt: &str) -> Result<(), String> {
    let runtime = app.state::<RuntimeState>();
    let mut state = runtime.0.lock().expect("state lock");
    let prompt = prompt.trim().to_string();
    state.last_prompt = prompt.clone();
    state.remember_prompt(&prompt);
    if !prompt.is_empty() {
        state.recent_messages.push(state::ConversationMessage {
            author: "You".into(),
            text: prompt,
        });
    }
    state::save_state(&state).map_err(|err| err.to_string())
}

pub fn record_jcode_response(
    app: &AppHandle,
    output: &str,
    session_id: Option<String>,
    token_stats: Option<state::TokenStats>,
) -> Result<(), String> {
    let runtime = app.state::<RuntimeState>();
    let mut state = runtime.0.lock().expect("state lock");
    state.recent_messages.push(state::ConversationMessage {
        author: "jcode".into(),
        text: output.to_string(),
    });
    if let Some(session_id) = session_id {
        state.active_session = Some(session_id);
    }
    if let Some(token_stats) = token_stats {
        state.token_stats = Some(token_stats);
    }
    state::save_state(&state).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use crate::core::activity;

    #[test]
    fn status_constants_match_python_style_states() {
        assert_eq!(activity::SENDING_STATUS, "sending");
        assert_eq!(activity::ANSWERING_STATUS, "answering");
        assert_eq!(activity::ERROR_STATUS, "error");
    }
}
