use crate::core::{formatting, positioning, state::TokenStats};
use serde::Serialize;
use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

static PROMPT_TRACKING: Mutex<PromptTrackingState> = Mutex::new(PromptTrackingState {
    current_x: None,
    current_y: None,
});
static PROMPT_TRACKING_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_FEEDBACK: Mutex<Option<FeedbackPayload>> = Mutex::new(None);

#[derive(Debug, Clone, Copy)]
struct PromptTrackingState {
    current_x: Option<f64>,
    current_y: Option<f64>,
}

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

fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("missing window {label}"))?;
    window.hide().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn show_prompt(app: AppHandle) -> Result<(), String> {
    show_prompt_window(&app)
}

pub fn show_prompt_window(app: &AppHandle) -> Result<(), String> {
    reset_prompt_tracking();
    if let Some(window) = app.get_webview_window("prompt") {
        place_prompt_at_mouse_or_center(&window);
    }
    let result = show_window(app, "prompt");
    if result.is_ok() {
        start_prompt_mouse_follow(app);
        if let Some(window) = app.get_webview_window("prompt") {
            let _ = window.emit("prompt-shown", ());
        }
    }
    result
}

fn start_prompt_mouse_follow(app: &AppHandle) {
    stop_prompt_mouse_follow();
    PROMPT_TRACKING_ACTIVE.store(true, Ordering::SeqCst);
    let app = app.clone();
    thread::spawn(move || {
        while PROMPT_TRACKING_ACTIVE.load(Ordering::SeqCst) {
            let Some((x, y)) = mouse_position() else {
                thread::sleep(Duration::from_millis(16));
                continue;
            };
            if x <= 2 && y <= 2 {
                thread::sleep(Duration::from_millis(16));
                continue;
            }
            let target = ((x + 20).max(0) as f64, (y + 24).max(0) as f64);
            let next = next_prompt_position(target).unwrap_or(target);
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                let Some(window) = app_for_main.get_webview_window("prompt") else {
                    PROMPT_TRACKING_ACTIVE.store(false, Ordering::SeqCst);
                    return;
                };
                if !window.is_visible().unwrap_or(false) {
                    PROMPT_TRACKING_ACTIVE.store(false, Ordering::SeqCst);
                    reset_prompt_tracking();
                    return;
                }
                let _ = window.set_position(PhysicalPosition::new(next.0 as i32, next.1 as i32));
            });
            thread::sleep(Duration::from_millis(16));
        }
        reset_prompt_tracking();
    });
}

fn stop_prompt_mouse_follow() {
    PROMPT_TRACKING_ACTIVE.store(false, Ordering::SeqCst);
}

#[tauri::command]
pub fn prompt_follow_mouse_tick(app: AppHandle) -> Result<bool, String> {
    // Compatibility command for older loaded frontends. Actual tracking is owned by Rust.
    Ok(app
        .get_webview_window("prompt")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false))
}

fn next_prompt_position(target: (f64, f64)) -> Result<(f64, f64), String> {
    let mut tracking = PROMPT_TRACKING
        .lock()
        .map_err(|_| "prompt tracking lock poisoned".to_string())?;
    let next = match (tracking.current_x, tracking.current_y) {
        (Some(cx), Some(cy)) => {
            let alpha = 0.55;
            (
                smooth_step(cx, target.0, alpha),
                smooth_step(cy, target.1, alpha),
            )
        }
        _ => target,
    };
    tracking.current_x = Some(next.0);
    tracking.current_y = Some(next.1);
    Ok(next)
}

fn reset_prompt_tracking() {
    if let Ok(mut tracking) = PROMPT_TRACKING.lock() {
        tracking.current_x = None;
        tracking.current_y = None;
    }
}

fn smooth_step(current: f64, target: f64, alpha: f64) -> f64 {
    let next = current + (target - current) * alpha;
    if (target - next).abs() < 1.0 {
        target
    } else {
        next
    }
}

fn place_prompt_at_mouse_or_center(window: &tauri::WebviewWindow) {
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
pub fn hide_prompt(app: AppHandle) -> Result<(), String> {
    stop_prompt_mouse_follow();
    reset_prompt_tracking();
    hide_window(&app, "prompt")
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
    if let Ok(mut last) = LAST_FEEDBACK.lock() {
        *last = Some(payload.clone());
    }
    if let Some(window) = app.get_webview_window("feedback") {
        move_feedback_to_corner(&window);
        window.show().map_err(|err| err.to_string())?;
        let _ = window.emit("feedback-update", payload.clone());
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(120));
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = app_for_main.get_webview_window("feedback") {
                    let _ = window.emit("feedback-update", payload);
                }
            });
        });
    }
    Ok(())
}

#[tauri::command]
pub fn current_feedback() -> Option<FeedbackPayload> {
    LAST_FEEDBACK.lock().ok().and_then(|last| last.clone())
}

#[tauri::command]
pub fn hide_feedback(app: AppHandle) -> Result<(), String> {
    hide_window(&app, "feedback")
}

fn move_feedback_to_corner(window: &tauri::WebviewWindow) {
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
