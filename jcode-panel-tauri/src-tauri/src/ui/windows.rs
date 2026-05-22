use tauri::{AppHandle, Manager, WebviewWindow};

fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("missing window {label}"))?;
    window.show().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())?;
    Ok(())
}

fn hide_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn show_prompt(app: AppHandle) -> Result<(), String> {
    show_window(&app, "prompt")
}

#[tauri::command]
pub fn hide_prompt(window: WebviewWindow) -> Result<(), String> {
    hide_window(window)
}

#[tauri::command]
pub fn show_dropdown(app: AppHandle) -> Result<(), String> {
    show_window(&app, "dropdown")
}

#[tauri::command]
pub fn show_settings(app: AppHandle) -> Result<(), String> {
    show_window(&app, "settings")
}
