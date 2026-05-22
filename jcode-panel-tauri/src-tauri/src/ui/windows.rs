use crate::core::{formatting, positioning, state::TokenStats};
use serde::Serialize;
use std::{process::Command, thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackPayload {
    pub text: String,
    pub notice: String,
    pub stats: Option<TokenStats>,
}

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
    show_prompt_window(&app)
}

pub fn show_prompt_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("prompt") {
        place_prompt_at_mouse_or_center(&window);
    }
    show_window(app, "prompt")?;
    start_prompt_mouse_follow(app);
    Ok(())
}

fn start_prompt_mouse_follow(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let mut current: Option<(f64, f64)> = None;
        for _ in 0..90 {
            let Some(window) = app.get_webview_window("prompt") else {
                break;
            };
            if !window.is_visible().unwrap_or(false) {
                break;
            }
            let Some((x, y)) = mouse_position() else {
                thread::sleep(Duration::from_millis(16));
                continue;
            };
            if x <= 2 && y <= 2 {
                thread::sleep(Duration::from_millis(16));
                continue;
            }
            let target = ((x + 20).max(0) as f64, (y + 24).max(0) as f64);
            let next = match current {
                None => target,
                Some((cx, cy)) => {
                    let alpha = 0.55;
                    let nx = smooth_step(cx, target.0, alpha);
                    let ny = smooth_step(cy, target.1, alpha);
                    (nx, ny)
                }
            };
            current = Some(next);
            let _ = window.set_position(PhysicalPosition::new(next.0 as i32, next.1 as i32));
            thread::sleep(Duration::from_millis(16));
        }
    });
}

fn smooth_step(current: f64, target: f64, alpha: f64) -> f64 {
    let next = current + (target - current) * alpha;
    if (target - next).abs() < 1.0 {
        target
    } else {
        next
    }
}

fn place_prompt_at_mouse_or_center(window: &WebviewWindow) {
    if let Some((x, y)) = mouse_position() {
        if x > 2 || y > 2 {
            let _ = window.set_position(PhysicalPosition::new((x + 20).max(0), (y + 24).max(0)));
            return;
        }
    }
    let _ = window.center();
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

#[tauri::command]
pub fn show_feedback(
    app: AppHandle,
    text: String,
    notice: Option<String>,
    stats: Option<TokenStats>,
) -> Result<(), String> {
    show_feedback_window(&app, &text, notice.as_deref().unwrap_or(""), stats)
}

pub fn show_feedback_window(
    app: &AppHandle,
    text: &str,
    notice: &str,
    stats: Option<TokenStats>,
) -> Result<(), String> {
    let text = formatting::format_stream_lines(text, 9);
    let payload = FeedbackPayload {
        text,
        notice: notice.trim().to_string(),
        stats,
    };
    if let Some(window) = app.get_webview_window("feedback") {
        move_feedback_to_corner(&window);
        window.show().map_err(|err| err.to_string())?;
        let _ = window.emit("feedback-update", payload);
    }
    Ok(())
}

#[tauri::command]
pub fn hide_feedback(window: WebviewWindow) -> Result<(), String> {
    hide_window(window)
}

fn move_feedback_to_corner(window: &WebviewWindow) {
    if let Some(monitor) = window.current_monitor().ok().flatten() {
        let pos = monitor.position();
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let window_size = window.outer_size().ok();
        let width = window_size.map(|s| s.width as f64 / scale).unwrap_or(540.0) as i32;
        let height = window_size
            .map(|s| s.height as f64 / scale)
            .unwrap_or(300.0) as i32;
        let x = pos.x + size.width as i32 - width - 24;
        let y = pos.y + size.height as i32 - height - 48;
        let _ = window.set_position(PhysicalPosition::new(x.max(0), y.max(0)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_step_snaps_when_close() {
        assert_eq!(smooth_step(10.0, 10.5, 0.55), 10.5);
    }

    #[test]
    fn smooth_step_moves_toward_target() {
        let next = smooth_step(0.0, 100.0, 0.55);
        assert!(next > 50.0 && next < 60.0);
    }
}
