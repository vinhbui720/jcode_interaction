use crate::{
    core::{activity, protocol, state},
    ui::commands::RuntimeState,
};
use std::{thread, time::Duration};
use tauri::{AppHandle, Manager};

pub fn set_process_status(app: &AppHandle, process_status: &str) -> Result<(), String> {
    let status = process_status.trim();
    let status = if status.is_empty() {
        activity::IDLE_STATUS
    } else {
        status
    };
    let header = {
        let runtime = app.state::<RuntimeState>();
        let mut state = runtime.0.lock().expect("state lock");
        state.process_status = status.to_string();
        state.live_activity = None;
        let header = header_for_state(&state);
        state::save_state(&state).map_err(|err| err.to_string())?;
        header
    };
    set_header_status(app, &header)
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
    set_header_status(app, &header)?;
    start_header_ticker(app);
    Ok(())
}

fn start_header_ticker(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let state = app
            .state::<RuntimeState>()
            .0
            .lock()
            .expect("state lock")
            .clone();
        let still_active = state
            .live_activity
            .as_ref()
            .map(|activity| activity.active)
            .unwrap_or(false);
        if !still_active {
            break;
        }
        let header = header_for_state(&state);
        let app_for_main = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = set_header_status(&app_for_main, &header);
        });
    });
}

pub fn refresh_header_status(app: &AppHandle) -> Result<(), String> {
    let state = app
        .state::<RuntimeState>()
        .0
        .lock()
        .expect("state lock")
        .clone();
    let header = header_for_state(&state);
    set_header_status(app, &header)
}

pub fn record_stream_event(app: &AppHandle, event: &protocol::PanelEvent) -> Result<(), String> {
    use protocol::PanelEventKind;
    match event.kind {
        PanelEventKind::Status | PanelEventKind::Progress | PanelEventKind::Tool => {
            let label = protocol::activity_label(event.raw.as_ref(), &event.text);
            let state_text = protocol::activity_state(event.raw.as_ref(), &event.text);
            let label = if label.trim().is_empty() {
                event.kind_label().to_string()
            } else {
                label
            };
            let state_text = if state_text.trim().is_empty() {
                event.kind_label().to_string()
            } else {
                state_text
            };
            let terminal = protocol::activity_is_terminal(event);
            let header = {
                let runtime = app.state::<RuntimeState>();
                let mut app_state = runtime.0.lock().expect("state lock");
                if terminal {
                    app_state.process_status = state_text.clone();
                    app_state.live_activity = None;
                } else {
                    let mut live = activity::LiveActivity::new(label, state_text.clone());
                    if let Some(existing) = app_state.live_activity.as_ref() {
                        if existing.active && existing.label == live.label {
                            live.started_at_ms = existing.started_at_ms;
                        }
                    }
                    app_state.process_status = state_text;
                    app_state.live_activity = Some(live);
                }
                let header = header_for_state(&app_state);
                state::save_state(&app_state).map_err(|err| err.to_string())?;
                header
            };
            set_header_status(app, &header)?;
        }
        PanelEventKind::Message if !event.text.trim().is_empty() => {
            let header = {
                let runtime = app.state::<RuntimeState>();
                let mut app_state = runtime.0.lock().expect("state lock");
                let mut live = activity::LiveActivity::new("jcode", activity::ANSWERING_STATUS);
                if let Some(existing) = app_state.live_activity.as_ref() {
                    if existing.active && existing.label == live.label {
                        live.started_at_ms = existing.started_at_ms;
                    }
                }
                app_state.process_status = activity::ANSWERING_STATUS.into();
                app_state.live_activity = Some(live);
                let header = header_for_state(&app_state);
                state::save_state(&app_state).map_err(|err| err.to_string())?;
                header
            };
            set_header_status(app, &header)?;
        }
        PanelEventKind::Error => {
            set_process_status(app, activity::ERROR_STATUS)?;
        }
        _ => {}
    }
    Ok(())
}

pub fn header_for_state(state: &state::AppState) -> String {
    if state
        .live_activity
        .as_ref()
        .map(|a| a.active)
        .unwrap_or(false)
    {
        return activity::header_status(&state.process_status, state.live_activity.as_ref());
    }
    if state.process_status.trim().is_empty() || state.process_status == activity::IDLE_STATUS {
        activity::ready_status(&state.ready_client_name())
    } else {
        activity::header_status(&state.process_status, None)
    }
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
