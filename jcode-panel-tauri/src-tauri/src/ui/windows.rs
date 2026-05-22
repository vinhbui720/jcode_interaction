use crate::core::{formatting, positioning, state::TokenStats};
use serde::Serialize;
use std::{process::Command, sync::Mutex, thread, time::Duration};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

static LAST_FEEDBACK: Mutex<Option<FeedbackPayload>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackPayload {
    pub text: String,
    pub notice: String,
    pub status: String,
    pub stats: Option<TokenStats>,
}

#[derive(Debug, Clone, Copy)]
enum PanelWindow {
    Dropdown,
    Prompt,
    Settings,
    Feedback,
}

impl PanelWindow {
    fn label(self) -> &'static str {
        match self {
            Self::Dropdown => "dropdown",
            Self::Prompt => "prompt",
            Self::Settings => "settings",
            Self::Feedback => "feedback",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Dropdown => "Jcode Interaction",
            Self::Prompt => "Jcode Prompt",
            Self::Settings => "Jcode Panel Settings",
            Self::Feedback => "Jcode Feedback",
        }
    }

    fn url(self) -> String {
        format!("index.html?window={}", self.label())
    }

    fn size(self) -> (f64, f64) {
        match self {
            Self::Dropdown => (420.0, 620.0),
            Self::Prompt => (720.0, 112.0),
            Self::Settings => (720.0, 520.0),
            Self::Feedback => (540.0, 300.0),
        }
    }

    fn is_overlay(self) -> bool {
        matches!(self, Self::Prompt | Self::Feedback)
    }
}

fn ensure_window(app: &AppHandle, kind: PanelWindow) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(kind.label()) {
        return Ok(window);
    }
    let (width, height) = kind.size();
    let mut builder =
        WebviewWindowBuilder::new(app, kind.label(), WebviewUrl::App(kind.url().into()))
            .title(kind.title())
            .inner_size(width, height)
            .visible(false)
            .resizable(!kind.is_overlay());

    if kind.is_overlay() {
        builder = builder
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true);
    } else {
        builder = builder.decorations(true);
    }

    builder.build().map_err(|err| err.to_string())
}

fn show_window(app: &AppHandle, kind: PanelWindow) -> Result<WebviewWindow, String> {
    let window = ensure_window(app, kind)?;
    window.show().map_err(|err| err.to_string())?;
    window.unminimize().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())?;
    Ok(window)
}

fn close_window(app: &AppHandle, kind: PanelWindow) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(kind.label()) {
        window.close().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn show_prompt(app: AppHandle) -> Result<(), String> {
    show_prompt_window(&app)
}

pub fn show_prompt_window(app: &AppHandle) -> Result<(), String> {
    let window = ensure_window(app, PanelWindow::Prompt)?;
    place_prompt_at_mouse_or_center(&window);
    window.show().map_err(|err| err.to_string())?;
    window.unminimize().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())?;
    let _ = window.emit("prompt-shown", ());
    Ok(())
}

#[tauri::command]
pub fn prompt_follow_mouse_tick(app: AppHandle) -> Result<bool, String> {
    // Compatibility command for older loaded frontends. Actual tracking is owned by Rust.
    Ok(app
        .get_webview_window("prompt")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false))
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
    if let Some((x, y)) = mouse_position(window).or_else(positioning::last_cursor) {
        if let Some(pos) = clamped_overlay_position(window, x + 20, y + 24) {
            let _ = window.set_position(pos);
            return;
        }
    }
    let _ = window.center();
}

fn mouse_position(window: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    // On XWayland, Tauri/WebKit cursor_position can report coordinates in the
    // wrong space for transparent overlay windows. Prefer xdotool's root-window
    // coordinates when X11 is available, then fall back to Tauri for native
    // Wayland or systems without xdotool.
    if std::env::var_os("DISPLAY").is_some() {
        if let Some(pos) = xdotool_mouse_position() {
            positioning::remember_cursor(pos);
            return Some(pos);
        }
    }
    if let Ok(pos) = window.cursor_position() {
        let pos = (pos.x.round() as i32, pos.y.round() as i32);
        positioning::remember_cursor(pos);
        return Some(pos);
    }
    xdotool_mouse_position().inspect(|pos| positioning::remember_cursor(*pos))
}

fn xdotool_mouse_position() -> Option<(i32, i32)> {
    let output = Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let (x, y) = positioning::parse_xdotool_mouselocation(&text);
    Some((x?, y?))
}

fn clamped_overlay_position(
    window: &tauri::WebviewWindow,
    desired_x: i32,
    desired_y: i32,
) -> Option<PhysicalPosition<i32>> {
    let monitor = monitor_for_point(window, desired_x, desired_y)
        .or_else(|| window.current_monitor().ok().flatten())?;
    let pos = monitor.position();
    let size = monitor.size();
    let window_size = window
        .outer_size()
        .ok()
        .unwrap_or_else(|| PhysicalSize::new(720_u32, 112_u32));
    let min_x = pos.x;
    let min_y = pos.y;
    let max_x = pos.x + size.width as i32 - window_size.width as i32;
    let max_y = pos.y + size.height as i32 - window_size.height as i32;
    Some(PhysicalPosition::new(
        desired_x.clamp(min_x, max_x.max(min_x)),
        desired_y.clamp(min_y, max_y.max(min_y)),
    ))
}

#[tauri::command]
pub fn hide_prompt(app: AppHandle) -> Result<(), String> {
    let result = close_window(&app, PanelWindow::Prompt);
    crate::app::reset_prompt_shortcut(&app);
    result
}

#[tauri::command]
pub fn show_dropdown(app: AppHandle) -> Result<(), String> {
    show_window(&app, PanelWindow::Dropdown).map(|_| ())
}

#[tauri::command]
pub fn show_settings(app: AppHandle) -> Result<(), String> {
    show_window(&app, PanelWindow::Settings).map(|_| ())
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
        if let Ok(window) = ensure_window(&app_for_main, PanelWindow::Feedback) {
            if !window.is_visible().unwrap_or(false) {
                move_feedback_to_mouse_screen(&window);
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
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
    close_window(&app, PanelWindow::Feedback)?;
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
    let (mouse_x, mouse_y) = mouse_position(window).or_else(positioning::last_cursor)?;
    monitor_for_point(window, mouse_x, mouse_y)
}

fn monitor_for_point(
    window: &tauri::WebviewWindow,
    mouse_x: i32,
    mouse_y: i32,
) -> Option<tauri::Monitor> {
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
