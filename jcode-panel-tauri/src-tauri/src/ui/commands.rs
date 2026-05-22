use crate::{
    core::{
        config, controller, conversation, diagnostics, formatting, interaction_context, jcode,
        popup_context, state, terminal,
    },
    integrations,
    ui::status,
};
use serde::{Deserialize, Serialize};
use std::{process::Command, sync::Mutex, thread};
use tauri::{AppHandle, Manager, State};

pub struct RuntimeState(pub Mutex<state::AppState>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub config: config::AppConfig,
    pub state: state::AppState,
    pub jcode_available: bool,
    pub conversation_preview: String,
    pub header_status: String,
}

#[tauri::command]
pub fn snapshot(runtime: State<RuntimeState>) -> AppSnapshot {
    let state = runtime.0.lock().expect("state lock").clone();
    let mut buffer = conversation::ConversationBuffer::new(100);
    for msg in &state.recent_messages {
        if msg.author == "You" {
            buffer.add_user(&msg.text);
        } else {
            buffer.messages.push((msg.author.clone(), msg.text.clone()));
        }
    }
    AppSnapshot {
        config: config::load_config(),
        header_status: crate::core::activity::header_status(
            &state.process_status,
            state.live_activity.as_ref(),
        ),
        state,
        jcode_available: jcode::jcode_available_cached(),
        conversation_preview: buffer.latest_preview(false),
    }
}

#[tauri::command]
pub fn save_settings(new_config: config::AppConfig, app: AppHandle) -> Result<(), String> {
    config::save_config(&new_config).map_err(|err| err.to_string())?;
    crate::app::reset_prompt_shortcut(&app);
    Ok(())
}

#[tauri::command]
pub fn available_terminals() -> Vec<String> {
    terminal::detected_terminals()
}

#[tauri::command]
pub fn submit_prompt(
    prompt: String,
    runtime: State<RuntimeState>,
) -> Result<jcode::SendResult, String> {
    let current_state = runtime.0.lock().expect("state lock").clone();
    let controller = controller::AppController::new(config::load_config(), current_state.clone());
    if prompt.chars().count() > controller.max_prompt_chars() {
        return Err(format!(
            "Prompt is longer than configured limit of {} characters",
            controller.max_prompt_chars()
        ));
    }
    let session = current_state.active_session.clone();
    let normalized = interaction_context::normalize_interaction_tags(&prompt);
    let ctx = crate::core::context::capture_active_context();
    let selected_for_chip = ctx
        .browser
        .as_ref()
        .and_then(|b| (!b.selected_text.trim().is_empty()).then_some(b.selected_text.as_str()))
        .unwrap_or(&ctx.selected_text);
    let popup_chips = popup_context::build_popup_context_chips(
        selected_for_chip,
        &[],
        &ctx.app,
        &ctx.window_title,
        "",
        None,
    );
    let with_popup_context = popup_context::expand_popup_context_chips(&normalized, &popup_chips);
    let expanded = interaction_context::expand_interaction_chips(&with_popup_context)?;
    let screenshots: Vec<String> = runtime
        .0
        .lock()
        .expect("state lock")
        .recent_messages
        .iter()
        .filter_map(|m| m.text.strip_prefix("screenshot: ").map(str::to_string))
        .collect();
    let outgoing_prompt = formatting::expand_pic_tags(&expanded, &screenshots);
    let (outgoing_prompt, _) = controller.build_prompt(&outgoing_prompt, None, true, false);
    let result =
        jcode::send_prompt(&outgoing_prompt, session.as_deref()).map_err(|err| err.to_string())?;
    let mut state = runtime.0.lock().expect("state lock");
    let user_prompt = prompt.clone();
    state.last_prompt = prompt;
    state.remember_prompt(&user_prompt);
    state.recent_messages.push(state::ConversationMessage {
        author: "You".into(),
        text: user_prompt.clone(),
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
pub fn submit_prompt_async(prompt: String, app: AppHandle) -> Result<(), String> {
    let current_state = app
        .state::<RuntimeState>()
        .0
        .lock()
        .expect("state lock")
        .clone();
    let controller = controller::AppController::new(config::load_config(), current_state.clone());
    if prompt.chars().count() > controller.max_prompt_chars() {
        return Err(format!(
            "Prompt is longer than configured limit of {} characters",
            controller.max_prompt_chars()
        ));
    }
    let notice = if current_state
        .active_session
        .as_deref()
        .unwrap_or("")
        .is_empty()
    {
        "Creating new jcode session..."
    } else {
        "Sending prompt to persistent jcode client..."
    };
    status::record_user_prompt(&app, &prompt)?;
    status::start_activity(&app, crate::core::activity::SENDING_STATUS, "jcode")?;
    let _ = crate::ui::windows::show_feedback_window(&app, notice, "Working", None);
    let app_for_worker = app.clone();
    thread::spawn(move || {
        let result = submit_prompt_background(&app_for_worker, prompt);
        if let Err(error) = result {
            let _ =
                status::set_process_status(&app_for_worker, crate::core::activity::ERROR_STATUS);
            let _ =
                crate::ui::windows::show_feedback_window(&app_for_worker, &error, "Error", None);
        }
    });
    Ok(())
}

fn submit_prompt_background(app: &AppHandle, prompt: String) -> Result<(), String> {
    let current_state = app
        .state::<RuntimeState>()
        .0
        .lock()
        .expect("state lock")
        .clone();
    let controller = controller::AppController::new(config::load_config(), current_state.clone());
    let session = current_state.active_session.clone();
    let normalized = interaction_context::normalize_interaction_tags(&prompt);
    let ctx = crate::core::context::capture_active_context();
    let selected_for_chip = ctx
        .browser
        .as_ref()
        .and_then(|b| (!b.selected_text.trim().is_empty()).then_some(b.selected_text.as_str()))
        .unwrap_or(&ctx.selected_text);
    let popup_chips = popup_context::build_popup_context_chips(
        selected_for_chip,
        &[],
        &ctx.app,
        &ctx.window_title,
        "",
        None,
    );
    let with_popup_context = popup_context::expand_popup_context_chips(&normalized, &popup_chips);
    let expanded = interaction_context::expand_interaction_chips(&with_popup_context)?;
    let screenshots: Vec<String> = app
        .state::<RuntimeState>()
        .0
        .lock()
        .expect("state lock")
        .recent_messages
        .iter()
        .filter_map(|m| m.text.strip_prefix("screenshot: ").map(str::to_string))
        .collect();
    let outgoing_prompt = formatting::expand_pic_tags(&expanded, &screenshots);
    let (outgoing_prompt, _) = controller.build_prompt(&outgoing_prompt, None, true, false);
    let result =
        jcode::send_prompt(&outgoing_prompt, session.as_deref()).map_err(|err| err.to_string())?;
    status::record_jcode_response(
        app,
        &result.output,
        result.session_id.clone(),
        result.token_stats.clone(),
    )?;
    let final_status = if result.ok {
        crate::core::activity::COMPLETE_STATUS
    } else {
        crate::core::activity::ERROR_STATUS
    };
    status::set_process_status(app, final_status)?;
    let notice = if result.ok {
        "jcode response complete"
    } else {
        "jcode returned an error"
    };
    let _ =
        crate::ui::windows::show_feedback_window(app, &result.output, notice, result.token_stats);
    Ok(())
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
pub fn normalize_prompt_text(text: String) -> serde_json::Value {
    let normalized = interaction_context::normalize_interaction_tags(&text);
    serde_json::json!({
        "text": normalized,
        "hints": interaction_context::interaction_token_hints(&text, None),
    })
}

#[tauri::command]
pub fn capture_screenshot(mode: String, runtime: State<RuntimeState>) -> Result<String, String> {
    let mode = mode.trim().to_ascii_lowercase();
    let dir = dirs::picture_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("jcode-panel");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.join(format!(
        "screenshot-{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_secs()
    ));
    let path_text = path.to_string_lossy().to_string();
    let mut command = if Command::new("gnome-screenshot")
        .arg("--version")
        .output()
        .is_ok()
    {
        let mut cmd = Command::new("gnome-screenshot");
        if mode == "area" {
            cmd.arg("-a");
        }
        cmd.arg("-f").arg(&path_text);
        cmd
    } else if Command::new("import").arg("-version").output().is_ok() {
        let mut cmd = Command::new("import");
        if mode != "area" {
            cmd.arg("-window").arg("root");
        }
        cmd.arg(&path_text);
        cmd
    } else {
        return Err("No screenshot tool found. Install gnome-screenshot or imagemagick.".into());
    };
    let output = command.output().map_err(|err| err.to_string())?;
    if !output.status.success() || !path.exists() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let tag = formatting::screenshot_tag(&path_text);
    let mut state = runtime.0.lock().expect("state lock");
    state.recent_messages.push(state::ConversationMessage {
        author: "panel".into(),
        text: format!("screenshot: {path_text}"),
    });
    state::save_state(&state).map_err(|err| err.to_string())?;
    Ok(tag)
}

#[tauri::command]
pub fn active_context_snapshot() -> crate::core::context::ActiveContext {
    crate::core::context::capture_active_context()
}

#[tauri::command]
pub fn popup_context_chips() -> Vec<popup_context::PopupContextChip> {
    let ctx = crate::core::context::capture_active_context();
    let selected_for_chip = ctx
        .browser
        .as_ref()
        .and_then(|b| (!b.selected_text.trim().is_empty()).then_some(b.selected_text.as_str()))
        .unwrap_or(&ctx.selected_text);
    popup_context::build_popup_context_chips(
        selected_for_chip,
        &[],
        &ctx.app,
        &ctx.window_title,
        "",
        None,
    )
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

#[tauri::command]
pub fn diagnostics_report() -> diagnostics::DiagnosticsReport {
    let jcode_available = jcode::jcode_available_cached();
    let vscode = integrations::vscode::status();
    let obsidian = integrations::obsidian::status();
    diagnostics::DiagnosticsReport {
        checks: vec![
            diagnostics::CheckResult {
                name: "jcode".into(),
                ok: jcode_available,
                message: if jcode_available {
                    "jcode command available"
                } else {
                    "jcode command missing"
                }
                .into(),
                fix: "Install jcode and ensure it is on PATH".into(),
            },
            diagnostics::CheckResult {
                name: "vscode".into(),
                ok: vscode.installed,
                message: vscode.message,
                fix: "Use Refresh integrations from the panel".into(),
            },
            diagnostics::CheckResult {
                name: "obsidian".into(),
                ok: obsidian.installed,
                message: obsidian.message,
                fix: "Open an Obsidian vault, then refresh integrations".into(),
            },
        ],
    }
}

#[tauri::command]
pub fn launch_terminal(command: Option<String>) -> Result<(), String> {
    let cfg = config::load_config();
    let command = command.unwrap_or_else(|| "jcode repl".into());
    let args = if cfg.terminal.contains("{cmd}") || cfg.terminal.contains("{quoted_cmd}") {
        terminal::render_command(&cfg.terminal, &command)
    } else {
        vec![
            cfg.terminal,
            "--".into(),
            "sh".into(),
            "-lc".into(),
            command,
        ]
    };
    let Some((program, rest)) = args.split_first() else {
        return Err("No terminal command configured".into());
    };
    Command::new(program)
        .args(rest)
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(())
}
