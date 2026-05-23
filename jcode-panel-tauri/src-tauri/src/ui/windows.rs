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
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

static LAST_FEEDBACK: Mutex<Option<FeedbackPayload>> = Mutex::new(None);
static PROMPT_FOLLOW_RUNNING: AtomicBool = AtomicBool::new(false);
static PROMPT_ESCAPE_GRAB_RUNNING: AtomicBool = AtomicBool::new(false);
const PROMPT_INPUT_FOCUS_SCRIPT: &str = r#"
window.focus();
document.body.setAttribute('tabindex', '-1');
document.body.focus({preventScroll:true});
const promptInput = document.querySelector('#prompt-input');
if (promptInput) {
  promptInput.removeAttribute('disabled');
  promptInput.focus({preventScroll:true});
  promptInput.setSelectionRange(promptInput.value.length, promptInput.value.length);
}
"#;
const PROMPT_REFOCUS_DELAYS_MS: [u64; 7] = [40, 120, 260, 520, 900, 1_400, 2_000];
const PROMPT_X11_CLICK_FOCUS_DELAYS_MS: [u64; 3] = [90, 240, 520];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptToggleAction {
    Show,
    Hide,
}

fn prompt_toggle_action(prompt_visible: bool) -> PromptToggleAction {
    if prompt_visible {
        PromptToggleAction::Hide
    } else {
        PromptToggleAction::Show
    }
}

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
            Self::Feedback => (640.0, 360.0),
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
    if !kind.is_overlay() {
        place_window_on_mouse_screen(&window);
    }
    window.show().map_err(|err| err.to_string())?;
    window.unminimize().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())?;
    activate_window_title(kind.title());
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

#[tauri::command]
pub fn toggle_prompt(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("prompt") {
        if prompt_toggle_action(window.is_visible().unwrap_or(false)) == PromptToggleAction::Hide {
            return hide_prompt(app);
        }
    }
    show_prompt_window(&app)
}

pub fn show_prompt_window(app: &AppHandle) -> Result<(), String> {
    let window = ensure_window(app, PanelWindow::Prompt)?;
    place_prompt_at_mouse_or_center(&window);
    let _ = window.set_focusable(true);
    window.show().map_err(|err| err.to_string())?;
    let _ = window.set_always_on_top(true);
    window.unminimize().map_err(|err| err.to_string())?;
    activate_window_title(PanelWindow::Prompt.title());
    window.set_focus().map_err(|err| err.to_string())?;
    activate_window_title(PanelWindow::Prompt.title());
    let _ = window.emit("prompt-shown", ());
    let _ = window.eval(PROMPT_INPUT_FOCUS_SCRIPT);
    grab_escape_while_prompt_visible(app.clone());
    refocus_prompt_after_show(app.clone());
    click_focus_prompt_input_after_show(app.clone());
    follow_prompt_while_visible(app.clone());
    Ok(())
}

fn refocus_prompt_after_show(app: AppHandle) {
    thread::spawn(move || {
        for delay in PROMPT_REFOCUS_DELAYS_MS {
            thread::sleep(Duration::from_millis(delay));
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                let Some(window) = app_for_main.get_webview_window("prompt") else {
                    return;
                };
                if !window.is_visible().unwrap_or(false) {
                    return;
                }
                let _ = window.set_focus();
                let _ = window.emit("prompt-shown", ());
                let _ = window.eval(PROMPT_INPUT_FOCUS_SCRIPT);
            });
        }
    });
}

fn click_focus_prompt_input_after_show(app: AppHandle) {
    thread::spawn(move || {
        for delay in PROMPT_X11_CLICK_FOCUS_DELAYS_MS {
            thread::sleep(Duration::from_millis(delay));
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                let Some(window) = app_for_main.get_webview_window("prompt") else {
                    return;
                };
                if !window.is_visible().unwrap_or(false) {
                    return;
                }
                click_focus_prompt_input_x11(PanelWindow::Prompt.title());
            });
        }
    });
}

fn place_window_on_mouse_screen(window: &tauri::WebviewWindow) {
    let monitor = monitor_at_mouse(window).or_else(|| window.current_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let pos = monitor.position();
        let size = monitor.size();
        let window_size = window
            .outer_size()
            .ok()
            .unwrap_or_else(|| PhysicalSize::new(720_u32, 520_u32));
        let x = pos.x + ((size.width as i32 - window_size.width as i32) / 2).max(0);
        let y = pos.y + ((size.height as i32 - window_size.height as i32) / 2).max(0);
        let _ = window.set_position(PhysicalPosition::new(x, y));
    } else {
        let _ = window.center();
    }
}

fn activate_window_title(title: &str) {
    if std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let script = format!(
        "wid=$(xdotool search --name '{}' 2>/dev/null | tail -n1) || exit 0; \
         [ -n \"$wid\" ] || exit 0; \
         xdotool windowmap \"$wid\" 2>/dev/null || true; \
         xdotool windowraise \"$wid\" 2>/dev/null || true; \
         xdotool windowactivate --sync \"$wid\" 2>/dev/null || true; \
         xdotool windowfocus \"$wid\" 2>/dev/null || true",
        title.replace('\'', "'\\''")
    );
    let _ = Command::new("sh").args(["-c", script.as_str()]).spawn();
}

fn click_focus_prompt_input_x11(title: &str) {
    if std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let script = format!(
        "wid=$(xdotool search --name '{}' 2>/dev/null | tail -n1) || exit 0; \
         [ -n \"$wid\" ] || exit 0; \
         eval $(xdotool getmouselocation --shell 2>/dev/null); ox=$X; oy=$Y; \
         eval $(xdotool getwindowgeometry --shell \"$wid\" 2>/dev/null); \
         tx=$((X + WIDTH / 2)); ty=$((Y + 42)); \
         xdotool windowactivate --sync \"$wid\" 2>/dev/null || true; \
         xdotool windowfocus \"$wid\" 2>/dev/null || true; \
         xdotool mousemove \"$tx\" \"$ty\" click 1 2>/dev/null || true; \
         [ -n \"$ox\" ] && [ -n \"$oy\" ] && xdotool mousemove \"$ox\" \"$oy\" 2>/dev/null || true",
        title.replace('\'', "'\\''")
    );
    let _ = Command::new("sh").args(["-c", script.as_str()]).spawn();
}

#[tauri::command]
pub fn prompt_follow_mouse_tick(app: AppHandle) -> Result<bool, String> {
    let Some(window) = app.get_webview_window("prompt") else {
        return Ok(false);
    };
    let visible = window.is_visible().unwrap_or(false);
    if visible {
        place_prompt_at_mouse_or_center(&window);
    }
    Ok(visible)
}

fn follow_prompt_while_visible(app: AppHandle) {
    if PROMPT_FOLLOW_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(move || loop {
        let app_for_main = app.clone();
        let still_visible = app
            .run_on_main_thread(move || {
                let Some(window) = app_for_main.get_webview_window("prompt") else {
                    PROMPT_FOLLOW_RUNNING.store(false, Ordering::SeqCst);
                    return;
                };
                if window.is_visible().unwrap_or(false) {
                    place_prompt_at_mouse_or_center(&window);
                    // Desired modal-keyboard behavior: while the prompt is open,
                    // keep keyboard focus on the prompt so keystrokes do not leak
                    // into the previously focused app. Mouse clicks can still be
                    // made, but the prompt immediately reclaims keyboard focus.
                    let _ = window.set_focus();
                    let _ = window.eval(PROMPT_INPUT_FOCUS_SCRIPT);
                } else {
                    PROMPT_FOLLOW_RUNNING.store(false, Ordering::SeqCst);
                }
            })
            .is_ok();
        if !still_visible || !PROMPT_FOLLOW_RUNNING.load(Ordering::SeqCst) {
            PROMPT_FOLLOW_RUNNING.store(false, Ordering::SeqCst);
            break;
        }
        thread::sleep(Duration::from_millis(33));
    });
}

pub fn stop_prompt_follow() {
    PROMPT_FOLLOW_RUNNING.store(false, Ordering::SeqCst);
}

fn grab_escape_while_prompt_visible(app: AppHandle) {
    if PROMPT_ESCAPE_GRAB_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(move || {
        let result = run_x11_escape_grab(app.clone());
        if result.is_err() {
            PROMPT_ESCAPE_GRAB_RUNNING.store(false, Ordering::SeqCst);
        }
    });
}

fn run_x11_escape_grab(app: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::{
        connection::Connection,
        protocol::{
            xproto::{ConnectionExt, GrabMode, ModMask},
            Event,
        },
    };

    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;
    let escape_keycode = 9_u8;
    // Use ANY modifier rather than hand-picked NumLock/CapsLock combinations.
    // GNOME/XWayland can expose extra effective modifier bits, and then an Esc
    // press would bypass the grab even while the prompt owns focus.
    let _ = conn.grab_key(
        false,
        root,
        ModMask::ANY,
        escape_keycode,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    );
    let _ = conn.flush();

    while PROMPT_ESCAPE_GRAB_RUNNING.load(Ordering::SeqCst) {
        let visible = app
            .get_webview_window("prompt")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        if !visible {
            break;
        }
        if let Some(Event::KeyPress(event)) = conn.poll_for_event()? {
            if event.detail != escape_keycode {
                continue;
            }
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = hide_prompt(app_for_main);
            });
            break;
        }
        thread::sleep(Duration::from_millis(16));
    }
    let _ = conn.ungrab_key(escape_keycode, root, ModMask::ANY);
    let _ = conn.flush();
    PROMPT_ESCAPE_GRAB_RUNNING.store(false, Ordering::SeqCst);
    Ok(())
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
    if let Some(pos) = gnome_cursor_position() {
        positioning::remember_cursor(pos);
        return Some(pos);
    }
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

fn gnome_cursor_position() -> Option<(i32, i32)> {
    if std::env::var("XDG_CURRENT_DESKTOP")
        .map(|desktop| !desktop.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(true)
    {
        return None;
    }
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.jcode.Panel.Cursor",
            "--object-path",
            "/org/jcode/Panel/Cursor",
            "--method",
            "org.jcode.Panel.Cursor.GetPosition",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    positioning::parse_gdbus_int_pair(&text)
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
    stop_prompt_follow();
    PROMPT_ESCAPE_GRAB_RUNNING.store(false, Ordering::SeqCst);
    let result = if let Some(window) = app.get_webview_window(PanelWindow::Prompt.label()) {
        // Keep the prompt webview alive between toggles. Closing and rebuilding
        // the transparent WebKit window can race frontend listener setup, so a
        // repeated open may appear without the input owning keyboard focus.
        window.hide().map_err(|err| err.to_string())
    } else {
        Ok(())
    };
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
    let monitor = monitor_at_focused_window(window)
        .or_else(|| monitor_at_mouse(window))
        .or_else(|| window.current_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let pos = monitor.position();
        let size = monitor.size();
        let window_size = window
            .outer_size()
            .ok()
            .unwrap_or_else(|| PhysicalSize::new(640_u32, 360_u32));
        // Presentation only: make feedback feel like a notification sliding down
        // from the top/header area instead of a bottom toast.
        let x = pos.x + ((size.width as i32 - window_size.width as i32) / 2).max(0);
        let y = pos.y + 42;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

fn monitor_at_focused_window(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    let (x, y) = active_window_center()?;
    monitor_for_point(window, x, y)
}

fn active_window_center() -> Option<(i32, i32)> {
    if std::env::var_os("DISPLAY").is_none() {
        return None;
    }
    let output = Command::new("sh")
        .args([
            "-c",
            "wid=$(xdotool getactivewindow 2>/dev/null) || exit 1; xdotool getwindowgeometry --shell \"$wid\" 2>/dev/null",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let (x, y, width, height) = parse_xdotool_window_geometry(&text);
    Some((x? + width? / 2, y? + height? / 2))
}

fn parse_xdotool_window_geometry(
    text: &str,
) -> (Option<i32>, Option<i32>, Option<i32>, Option<i32>) {
    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("X=") {
            x = value.trim().parse().ok();
        } else if let Some(value) = line.strip_prefix("Y=") {
            y = value.trim().parse().ok();
        } else if let Some(value) = line.strip_prefix("WIDTH=") {
            width = value.trim().parse().ok();
        } else if let Some(value) = line.strip_prefix("HEIGHT=") {
            height = value.trim().parse().ok();
        }
    }
    (x, y, width, height)
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
    fn prompt_toggle_hides_when_prompt_is_visible() {
        assert_eq!(prompt_toggle_action(true), PromptToggleAction::Hide);
    }

    #[test]
    fn prompt_toggle_shows_when_prompt_is_missing_or_hidden() {
        assert_eq!(prompt_toggle_action(false), PromptToggleAction::Show);
    }

    #[test]
    fn prompt_window_is_overlay_so_builder_keeps_it_always_on_top() {
        assert!(PanelWindow::Prompt.is_overlay());
        assert_eq!(PanelWindow::Prompt.label(), "prompt");
        assert_eq!(PanelWindow::Prompt.title(), "Jcode Prompt");
    }

    #[test]
    fn parses_xdotool_window_geometry() {
        let (x, y, width, height) = parse_xdotool_window_geometry(
            "WINDOW=123\nX=1920\nY=24\nWIDTH=1280\nHEIGHT=720\nSCREEN=0\n",
        );
        assert_eq!(
            (x, y, width, height),
            (Some(1920), Some(24), Some(1280), Some(720))
        );
    }

    #[test]
    fn prompt_refocus_targets_prompt_input_repeatedly_after_show() {
        assert_eq!(
            PROMPT_REFOCUS_DELAYS_MS,
            [40, 120, 260, 520, 900, 1_400, 2_000]
        );
        assert!(PROMPT_INPUT_FOCUS_SCRIPT.contains("#prompt-input"));
        assert!(PROMPT_INPUT_FOCUS_SCRIPT.contains("preventScroll:true"));
    }

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
