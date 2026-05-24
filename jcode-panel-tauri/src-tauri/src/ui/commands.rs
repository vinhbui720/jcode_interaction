use crate::{
    core::{
        config, controller, conversation, diagnostics, formatting, interaction_context, jcode,
        popup_context, protocol, state, terminal, tts,
    },
    integrations,
    ui::status,
};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::{process::Command, sync::Mutex, thread};
use tauri::{AppHandle, Manager, State};

pub struct RuntimeState(pub Mutex<state::AppState>);

static LAST_POPUP_CONTEXT_CHIPS: Mutex<Vec<popup_context::PopupContextChip>> =
    Mutex::new(Vec::new());

fn store_popup_context_chips(
    chips: Vec<popup_context::PopupContextChip>,
) -> Vec<popup_context::PopupContextChip> {
    if let Ok(mut cached) = LAST_POPUP_CONTEXT_CHIPS.lock() {
        *cached = chips.clone();
    }
    chips
}

fn cached_popup_context_chips() -> Vec<popup_context::PopupContextChip> {
    LAST_POPUP_CONTEXT_CHIPS
        .lock()
        .map(|chips| chips.clone())
        .unwrap_or_default()
}

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
        header_status: crate::ui::status::header_for_state(&state),
        state,
        jcode_available: jcode::jcode_available_cached(),
        conversation_preview: buffer.latest_preview(false),
    }
}

#[tauri::command]
pub fn save_settings(new_config: config::AppConfig, app: AppHandle) -> Result<(), String> {
    if !prompt_hotkey_supported(&new_config.prompt_hotkey) {
        return Err(format!(
            "Prompt hotkey '{}' is not supported. Use a keyboard shortcut like F8/Super+Z or a GNOME mouse shortcut like Mouse8/Mouse9.",
            new_config.prompt_hotkey
        ));
    }
    config::save_config(&new_config).map_err(|err| err.to_string())?;
    sync_gnome_prompt_shortcut(&new_config.prompt_hotkey);
    crate::app::reset_prompt_shortcut(&app);
    Ok(())
}

#[tauri::command]
pub fn set_tts_enabled(enabled: bool, app: AppHandle) -> Result<config::AppConfig, String> {
    let mut cfg = config::load_config();
    cfg.tts_enabled = enabled;
    config::save_config(&cfg).map_err(|err| err.to_string())?;
    crate::app::reset_prompt_shortcut(&app);
    Ok(cfg)
}

#[tauri::command]
pub fn speak_feedback_text(text: String) -> Result<(), String> {
    let cfg = config::load_config();
    tts::speak_feedback_async(cfg, text);
    Ok(())
}

#[tauri::command]
pub fn suspend_prompt_hotkey(app: AppHandle) -> Result<(), String> {
    crate::app::suspend_prompt_shortcut(&app);
    Ok(())
}

#[tauri::command]
pub fn resume_prompt_hotkey(app: AppHandle) -> Result<(), String> {
    crate::app::reset_prompt_shortcut(&app);
    Ok(())
}

#[tauri::command]
pub fn quit_app() {
    std::process::exit(0);
}

fn prompt_hotkey_supported(hotkey: &str) -> bool {
    crate::app::prompt_hotkey_supported(hotkey) || gnome_binding(hotkey).is_some()
}

pub fn sync_gnome_prompt_shortcut(hotkey: &str) {
    let Some(binding) = gnome_binding(hotkey) else {
        return;
    };
    let base =
        "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/jcode-panel-prompt/";
    let media_schema = "org.gnome.settings-daemon.plugins.media-keys";
    let media_key = "custom-keybindings";
    if let Ok(output) = Command::new("gsettings")
        .args(["get", media_schema, media_key])
        .output()
    {
        let current = String::from_utf8_lossy(&output.stdout);
        let mut entries: Vec<String> = if current.trim() == "@as []" {
            Vec::new()
        } else {
            current
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|entry| entry.trim().trim_matches('\'').to_string())
                .filter(|entry| !entry.is_empty())
                .collect()
        };
        if !entries.iter().any(|entry| entry == base) {
            entries.push(base.into());
            let rendered = format!(
                "[{}]",
                entries
                    .iter()
                    .map(|entry| format!("'{}'", entry.replace('\'', "\\'")))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let _ = Command::new("gsettings")
                .args(["set", media_schema, media_key, &rendered])
                .status();
        }
    }
    let schema = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/jcode-panel-prompt/";
    let _ = Command::new("gsettings")
        .args(["set", schema, "binding", &binding])
        .status();
    let command = dirs::home_dir()
        .map(|home| format!("{} prompt", home.join(".local/bin/jcp").to_string_lossy()))
        .unwrap_or_else(|| "jcp prompt".into());
    let _ = Command::new("gsettings")
        .args(["set", schema, "command", &command])
        .status();
    let _ = Command::new("gsettings")
        .args(["set", schema, "name", "Jcode Interaction"])
        .status();
}

fn gnome_binding(hotkey: &str) -> Option<String> {
    let (mods, key) = crate::core::hotkeys::hotkey_parts(hotkey);
    if key.is_empty() {
        return None;
    }
    let mut out = String::new();
    if mods.contains("ctrl") {
        out.push_str("<Primary>");
    }
    if mods.contains("alt") {
        out.push_str("<Alt>");
    }
    if mods.contains("shift") {
        out.push_str("<Shift>");
    }
    if mods.contains("super") {
        out.push_str("<Super>");
    }
    if let Some(mouse) = key.strip_prefix("mouse") {
        out.push_str("Button");
        out.push_str(mouse);
    } else {
        out.push_str(&key);
    }
    Some(out)
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
    let popup_chips = cached_popup_context_chips();
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
    let popup_chips = cached_popup_context_chips();
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
    let mut live_feedback = conversation::ConversationBuffer::new(20);
    let app_for_events = app.clone();
    let result = jcode::send_prompt_streaming(&outgoing_prompt, session.as_deref(), move |event| {
        let _ = status::record_stream_event(&app_for_events, &event);
        let notice = protocol::event_preview(&event, false);
        live_feedback.add_event(&event);
        let text = live_feedback
            .messages
            .iter()
            .rev()
            .find(|(who, text)| who == "jcode" && !text.trim().is_empty())
            .map(|(_, text)| formatting::clean_feedback_text(text))
            .unwrap_or_else(|| notice.clone());
        if !text.trim().is_empty() || !notice.trim().is_empty() {
            let _ = crate::ui::windows::show_feedback_window(&app_for_events, &text, &notice, None);
        }
    })
    .map_err(|err| err.to_string())?;
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
    app: AppHandle,
    runtime: State<RuntimeState>,
) -> Result<state::AppState, String> {
    let updated = {
        let mut state = runtime.0.lock().expect("state lock");
        state.active_session = Some(session.trim().to_string()).filter(|s| !s.is_empty());
        if let Some(name) = name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
        {
            state.active_section = name;
        }
        state::save_state(&state).map_err(|err| err.to_string())?;
        state.clone()
    };
    status::refresh_header_status(&app)?;
    Ok(updated)
}

#[tauri::command]
pub fn start_new_section(
    name: Option<String>,
    app: AppHandle,
    runtime: State<RuntimeState>,
) -> Result<state::AppState, String> {
    let updated = {
        let mut state = runtime.0.lock().expect("state lock");
        state.active_session = None;
        state.active_section = name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Fresh Panel".into());
        state.recent_messages.clear();
        state::save_state_to_path_preserving(&state, &state::state_path(), true)
            .map_err(|err| err.to_string())?;
        state.clone()
    };
    status::refresh_header_status(&app)?;
    Ok(updated)
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
pub fn capture_screenshot(
    mode: String,
    app: AppHandle,
    runtime: State<RuntimeState>,
) -> Result<String, String> {
    let mode = mode.trim().to_ascii_lowercase();
    let restore_prompt = mode == "area";
    if restore_prompt {
        let _ = crate::ui::windows::hide_prompt(app.clone());
        thread::sleep(std::time::Duration::from_millis(220));
    }
    let result = capture_screenshot_inner(&mode, runtime);
    if restore_prompt {
        let _ = crate::ui::windows::show_prompt_window(&app);
    }
    result
}

#[tauri::command]
pub fn image_data_url(path: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(path);
    let bytes = std::fs::read(&path).map_err(|err| err.to_string())?;
    let mime = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Ok(format!(
        "data:{mime};base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
}

fn capture_screenshot_inner(mode: &str, runtime: State<RuntimeState>) -> Result<String, String> {
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
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("screenshot command exited with status {}", output.status)
        };
        return Err(detail);
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
    let cached = cached_popup_context_chips();
    if !cached.is_empty() {
        return cached;
    }
    refresh_popup_context_chips()
}

pub fn refresh_popup_context_chips() -> Vec<popup_context::PopupContextChip> {
    let ctx = crate::core::context::capture_active_context();
    let chips = build_selected_context_chips(&ctx);
    store_popup_context_chips(chips)
}

fn build_selected_context_chips(
    ctx: &crate::core::context::ActiveContext,
) -> Vec<popup_context::PopupContextChip> {
    let mut chips = vec![];
    let mut seq = 1;
    let app_lower = ctx.app.to_ascii_lowercase();
    let title_lower = ctx.window_title.to_ascii_lowercase();
    let focused_browser = ["firefox", "chrome", "chromium", "brave", "edge", "browser"]
        .iter()
        .any(|name| app_lower.contains(name) || title_lower.contains(name));
    let browser_selection_matches = ctx
        .browser
        .as_ref()
        .map(|browser| {
            !browser.selected_text.trim().is_empty()
                && normalized_selection_eq(&browser.selected_text, &ctx.selected_text)
        })
        .unwrap_or(false);
    let likely_browser_source =
        focused_browser || ctx.app.trim().is_empty() || browser_selection_matches;
    if focused_browser || ctx.app.trim().is_empty() {
        if let Some(browser) = &ctx.browser {
            if !browser.selected_text.trim().is_empty() {
                chips.push(selected_text_chip(
                    seq,
                    &browser.selected_text,
                    &ctx.app,
                    &ctx.window_title,
                    Some(browser),
                ));
                return chips;
            }
        }
    }
    for path in selected_file_paths(&ctx.selected_text) {
        chips.push(popup_context::PopupContextChip {
            tag: format!("[selected{seq}]"),
            body: format!("Context: selected file\npath: {path}"),
            kind: "selected-file".into(),
        });
        seq += 1;
    }
    let selected_text = ctx.selected_text.trim();
    if !selected_text.is_empty() && chips.is_empty() {
        chips.push(selected_text_chip(
            seq,
            selected_text,
            &ctx.app,
            &ctx.window_title,
            ctx.browser.as_ref().filter(|_| likely_browser_source),
        ));
    }
    chips
}

fn normalized_selection_eq(left: &str, right: &str) -> bool {
    let normalize = |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
    let left = normalize(left);
    let right = normalize(right);
    !left.is_empty() && left == right
}

fn selected_text_chip(
    seq: usize,
    text: &str,
    app: &str,
    window_title: &str,
    browser: Option<&crate::core::context::BrowserContext>,
) -> popup_context::PopupContextChip {
    let mut parts = vec!["Context: selected text".to_string()];
    if !app.trim().is_empty() {
        parts.push(format!("app: {}", app.trim()));
    }
    if !window_title.trim().is_empty() {
        parts.push(format!("window: {}", window_title.trim()));
    }
    if let Some(browser) = browser {
        if !browser.title.trim().is_empty() {
            parts.push(format!("tab: {}", browser.title.trim()));
        }
        if !browser.url.trim().is_empty() {
            parts.push(format!("url: {}", browser.url.trim()));
        }
        if let Some(line) = browser.selection_line {
            parts.push(format!("line: {line}"));
        }
        if !browser.selection_context.trim().is_empty() {
            parts.push(format!("near: {}", browser.selection_context.trim()));
        }
    }
    parts.push("selected text:".into());
    parts.push(text.trim().into());
    popup_context::PopupContextChip {
        tag: format!("[selected{seq}]"),
        body: parts.join("\n"),
        kind: "selected-text".into(),
    }
}

fn selected_file_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let raw = line.trim().trim_matches(['\'', '"']);
            let raw = raw.strip_prefix("file://").unwrap_or(raw);
            let raw = raw.replace("%20", " ");
            let path = std::path::Path::new(&raw);
            (path.is_absolute() && path.exists()).then_some(raw)
        })
        .collect()
}

#[tauri::command]
pub fn integration_status() -> serde_json::Value {
    serde_json::json!({
        "vscode": integrations::vscode::status(),
        "obsidian": integrations::obsidian::status(),
        "browser": integrations::browser::status()
    })
}

#[tauri::command]
pub fn refresh_integrations() -> serde_json::Value {
    serde_json::json!({
        "vscode": integrations::vscode::install(),
        "obsidian": integrations::obsidian::install(),
        "browser": integrations::browser::install()
    })
}

#[tauri::command]
pub fn diagnostics_report() -> diagnostics::DiagnosticsReport {
    let jcode_available = jcode::jcode_available_cached();
    let vscode = integrations::vscode::status();
    let obsidian = integrations::obsidian::status();
    let browser = integrations::browser::status();
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
            diagnostics::CheckResult {
                name: "browser".into(),
                ok: browser.installed,
                message: browser.message,
                fix: "Refresh integrations, then restart Firefox once".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_binding_formats_keyboard_shortcut() {
        assert_eq!(
            gnome_binding("Ctrl+Alt+F8"),
            Some("<Primary><Alt>f8".into())
        );
    }

    #[test]
    fn gnome_binding_formats_mouse_shortcut() {
        assert_eq!(gnome_binding("Super+Mouse8"), Some("<Super>Button8".into()));
    }

    #[test]
    fn prompt_hotkey_accepts_keyboard_or_gnome_mouse_shortcut() {
        assert!(prompt_hotkey_supported("Super+Z"));
        assert!(prompt_hotkey_supported("Mouse9"));
        assert!(prompt_hotkey_supported("Super+Mouse8"));
    }

    #[test]
    fn selected_browser_context_includes_tab_url_line_and_nearby_text() {
        let ctx = crate::core::context::ActiveContext {
            app: "firefox".into(),
            window_title: "Example page".into(),
            browser: Some(crate::core::context::BrowserContext {
                title: "Docs tab".into(),
                url: "https://example.test/docs".into(),
                selected_text: "selected phrase".into(),
                selection_line: Some(42),
                selection_context: "Some nearby selected phrase context".into(),
            }),
            selected_text: "selected phrase".into(),
            clipboard_text: String::new(),
        };
        let chips = build_selected_context_chips(&ctx);
        assert_eq!(chips.len(), 1);
        let body = &chips[0].body;
        assert!(body.contains("app: firefox"));
        assert!(body.contains("window: Example page"));
        assert!(body.contains("tab: Docs tab"));
        assert!(body.contains("url: https://example.test/docs"));
        assert!(body.contains("line: 42"));
        assert!(body.contains("near: Some nearby selected phrase context"));
        assert!(body.contains("selected text:\nselected phrase"));
    }

    #[test]
    fn selected_primary_text_reuses_matching_browser_source_metadata() {
        let ctx = crate::core::context::ActiveContext {
            app: "unknown-app".into(),
            window_title: "Unknown".into(),
            browser: Some(crate::core::context::BrowserContext {
                title: "Browser tab".into(),
                url: "https://example.test/page".into(),
                selected_text: "same text".into(),
                selection_line: Some(7),
                selection_context: "same text inside a paragraph".into(),
            }),
            selected_text: "same\ntext".into(),
            clipboard_text: String::new(),
        };
        let chips = build_selected_context_chips(&ctx);
        let body = &chips[0].body;
        assert!(body.contains("url: https://example.test/page"));
        assert!(body.contains("line: 7"));
    }
}
