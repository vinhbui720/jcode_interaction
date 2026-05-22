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
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

static PROMPT_TRACKING: Mutex<PromptTrackingState> = Mutex::new(PromptTrackingState {
    current_x: None,
    current_y: None,
});
static PROMPT_TRACKING_ACTIVE: AtomicBool = AtomicBool::new(false);
static PROMPT_FOCUS_ACTIVE: AtomicBool = AtomicBool::new(false);
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
    pub status: String,
    pub stats: Option<TokenStats>,
}

fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("missing window {label}"))?;
    window.show().map_err(|err| err.to_string())?;
    window.unminimize().map_err(|err| err.to_string())?;
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
        activate_prompt_window();
        start_prompt_focus_guard(app);
        start_prompt_mouse_follow(app);
        if let Some(window) = app.get_webview_window("prompt") {
            let _ = window.emit("prompt-shown", ());
        }
    }
    result
}

fn activate_prompt_window() {
    thread::spawn(|| {
        for delay in [20_u64, 80, 180] {
            thread::sleep(Duration::from_millis(delay));
            let _ = Command::new("xdotool")
                .args([
                    "search",
                    "--name",
                    "Jcode Prompt",
                    "windowactivate",
                    "--sync",
                    "windowfocus",
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    });
}

fn start_prompt_focus_guard(app: &AppHandle) {
    stop_prompt_focus_guard();
    PROMPT_FOCUS_ACTIVE.store(true, Ordering::SeqCst);
    let app = app.clone();
    thread::spawn(move || {
        for _ in 0..8 {
            if !PROMPT_FOCUS_ACTIVE.load(Ordering::SeqCst) {
                break;
            }
            let visible = app
                .get_webview_window("prompt")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            if !visible {
                PROMPT_FOCUS_ACTIVE.store(false, Ordering::SeqCst);
                break;
            }
            activate_prompt_once();
            thread::sleep(Duration::from_millis(80));
        }
        PROMPT_FOCUS_ACTIVE.store(false, Ordering::SeqCst);
    });
}

fn stop_prompt_focus_guard() {
    PROMPT_FOCUS_ACTIVE.store(false, Ordering::SeqCst);
}

fn activate_prompt_once() {
    let _ = Command::new("xdotool")
        .args([
            "search",
            "--name",
            "Jcode Prompt",
            "windowactivate",
            "--sync",
            "windowfocus",
            "--sync",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn start_prompt_mouse_follow(app: &AppHandle) {
    stop_prompt_mouse_follow();
    PROMPT_TRACKING_ACTIVE.store(true, Ordering::SeqCst);
    let app = app.clone();
    thread::spawn(move || {
        while PROMPT_TRACKING_ACTIVE.load(Ordering::SeqCst) {
            let visible = app
                .get_webview_window("prompt")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            if !visible {
                PROMPT_TRACKING_ACTIVE.store(false, Ordering::SeqCst);
                reset_prompt_tracking();
                break;
            }
            if let Some((x, y)) = mouse_position().filter(|(x, y)| *x > 2 || *y > 2) {
                let target = ((x + 20).max(0) as f64, (y + 24).max(0) as f64);
                if let Ok(Some(next)) = next_prompt_position_if_changed(target) {
                    let app_for_main = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Some(window) = app_for_main.get_webview_window("prompt") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.set_position(PhysicalPosition::new(
                                    next.0 as i32,
                                    next.1 as i32,
                                ));
                            }
                        }
                    });
                }
            }
            thread::sleep(Duration::from_millis(120));
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

fn reset_prompt_tracking() {
    if let Ok(mut tracking) = PROMPT_TRACKING.lock() {
        tracking.current_x = None;
        tracking.current_y = None;
    }
}

fn next_prompt_position_if_changed(target: (f64, f64)) -> Result<Option<(f64, f64)>, String> {
    let mut tracking = PROMPT_TRACKING
        .lock()
        .map_err(|_| "prompt tracking lock poisoned".to_string())?;
    let next = match (tracking.current_x, tracking.current_y) {
        (Some(cx), Some(cy)) => {
            if (target.0 - cx).abs() < 10.0 && (target.1 - cy).abs() < 10.0 {
                return Ok(None);
            }
            (
                smooth_step(cx, target.0, 0.45),
                smooth_step(cy, target.1, 0.45),
            )
        }
        _ => target,
    };
    tracking.current_x = Some(next.0);
    tracking.current_y = Some(next.1);
    Ok(Some(next))
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
    stop_prompt_focus_guard();
    stop_prompt_mouse_follow();
    reset_prompt_tracking();
    let result = hide_window(&app, "prompt");
    crate::app::reset_prompt_shortcut(&app);
    result
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
    let status = app
        .state::<crate::ui::commands::RuntimeState>()
        .0
        .lock()
        .ok()
        .map(|state| {
            crate::core::activity::header_status(
                &state.process_status,
                state.live_activity.as_ref(),
            )
        })
        .unwrap_or_else(|| "idle".into());
    let payload = FeedbackPayload {
        text,
        notice: notice.trim().to_string(),
        status,
        stats,
    };
    if let Ok(mut last) = LAST_FEEDBACK.lock() {
        *last = Some(payload.clone());
    }
    let app_for_main = app.clone();
    let payload_for_main = payload.clone();
    app.run_on_main_thread(move || {
        if let Some(window) = app_for_main.get_webview_window("feedback") {
            let was_visible = window.is_visible().unwrap_or(false);
            if !was_visible {
                move_feedback_to_mouse_screen(&window);
                let _ = window.show();
            }
            let _ = window.emit("feedback-update", payload_for_main);
        }
    })
    .map_err(|err| err.to_string())?;

    // Webviews can still be loading when the first streaming status arrives.
    // Replay on the main thread a few times, but never touch GTK/WebKit from the
    // worker thread that is reading the jcode stream. Calling show/set_position
    // off the UI thread can crash X11 with xcb_xlib_threads_sequence_lost.
    let app = app.clone();
    thread::spawn(move || {
        for delay in [120_u64, 350, 800] {
            thread::sleep(Duration::from_millis(delay));
            let app_for_main = app.clone();
            let payload_for_main = payload.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = app_for_main.get_webview_window("feedback") {
                    let _ = window.emit("feedback-update", payload_for_main);
                }
            });
        }
    });
    Ok(())
}

#[tauri::command]
pub fn current_feedback() -> Option<FeedbackPayload> {
    LAST_FEEDBACK.lock().ok().and_then(|last| last.clone())
}

#[tauri::command]
pub fn hide_feedback(app: AppHandle) -> Result<(), String> {
    hide_window(&app, "feedback")?;
    crate::ui::status::set_process_status(&app, crate::core::activity::IDLE_STATUS)?;
    Ok(())
}

fn move_feedback_to_mouse_screen(window: &tauri::WebviewWindow) {
    let monitor = monitor_at_mouse(window).or_else(|| window.current_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let pos = monitor.position();
        let size = monitor.size();
        let window_size = window
            .outer_size()
            .ok()
            .unwrap_or_else(|| PhysicalSize::new(540_u32, 300_u32));
        let x = pos.x + 24;
        let y = pos.y + size.height as i32 - window_size.height as i32 - 48;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

fn monitor_at_mouse(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let (mouse_x, mouse_y) = mouse_position()?;
    window
        .available_monitors()
        .ok()?
        .into_iter()
        .find(|monitor| {
            let pos = monitor.position();
            let size = monitor.size();
            mouse_x >= pos.x
                && mouse_y >= pos.y
                && mouse_x < pos.x + size.width as i32
                && mouse_y < pos.y + size.height as i32
        })
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
