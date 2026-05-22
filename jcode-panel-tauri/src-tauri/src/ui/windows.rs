use crate::core::positioning;
use std::process::Command;
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

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
    if let Some(window) = app.get_webview_window("prompt") {
        if let Some((x, y)) = mouse_position() {
            let _ = window.set_position(PhysicalPosition::new((x + 20).max(0), (y + 24).max(0)));
        }
    }
    show_window(&app, "prompt")
}

fn mouse_position() -> Option<(i32, i32)> {
    let output = Command::new("xdotool")
        .arg("getmouselocation")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let (x, y) = positioning::parse_xdotool_mouselocation(&text);
    Some((x?, y?))
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
