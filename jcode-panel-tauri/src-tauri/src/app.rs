use crate::{
    core::{config, context, hotkeys, state},
    integrations,
    ui::{
        commands::{self, RuntimeState},
        tray,
    },
};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process,
    sync::Mutex,
    thread,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn register_prompt_shortcut(app: &tauri::AppHandle) {
    let cfg = config::load_config();
    let shortcut = parse_prompt_shortcut_or_default(&cfg.prompt_hotkey);
    let handle = app.clone();
    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let _ = crate::ui::windows::show_prompt_window(&handle);
            }
        });
}

pub fn prompt_hotkey_supported(hotkey: &str) -> bool {
    parse_shortcut(hotkey).is_some()
}

fn parse_prompt_shortcut_or_default(hotkey: &str) -> Shortcut {
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return parse_shortcut("F8").expect("default F8 shortcut is valid");
    }
    parse_shortcut(hotkey)
        .unwrap_or_else(|| parse_shortcut("F8").expect("default F8 shortcut is valid"))
}

pub fn reset_prompt_shortcut(app: &tauri::AppHandle) {
    // On X11 a bare function-key shortcut can miss its release event when the
    // prompt appears immediately and takes focus. If the plugin keeps F8 marked
    // as pressed, later F8 presses are ignored. Re-registering after prompt hide
    // clears that plugin-side key state and matches the Python app's repeated
    // show/hide behavior.
    let _ = app.global_shortcut().unregister_all();
    register_prompt_shortcut(app);
}

fn lock_path() -> PathBuf {
    dirs::runtime_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("jcode-panel-tauri.pid")
}

fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("jcode-panel-tauri.sock")
}

fn cli_wants_prompt() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--prompt" || arg == "prompt" || arg == "--show")
}

fn cli_wants_settings() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--settings" || arg == "settings")
}

fn cli_wants_dropdown() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--dropdown" | "dropdown" | "--open" | "open"))
}

fn startup_command() -> Option<&'static str> {
    if cli_wants_settings() {
        Some("show_settings")
    } else if cli_wants_dropdown() {
        Some("show_dropdown")
    } else if cli_wants_prompt() {
        Some("show_prompt")
    } else {
        None
    }
}

fn send_request_to_running_instance(command: &str) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path()) else {
        return false;
    };
    let ok = stream.write_all(command.as_bytes()).is_ok() && stream.write_all(b"\n").is_ok();
    let _ = stream.shutdown(Shutdown::Write);
    ok
}

fn start_ipc_server(app: &tauri::AppHandle) {
    let path = socket_path();
    let _ = fs::remove_file(&path);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    let app = app.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            let mut reader = BufReader::new(stream);
            let mut command = String::new();
            let _ = reader.read_line(&mut command);
            let command = command.trim().to_string();
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || match command.as_str() {
                "show_settings" | "settings" | "--settings" => {
                    let _ = crate::ui::windows::show_settings(app_for_main.clone());
                }
                "show_dropdown" | "dropdown" | "--dropdown" | "open" | "--open" => {
                    let _ = crate::ui::windows::show_dropdown(app_for_main.clone());
                }
                _ => {
                    let _ = crate::ui::windows::show_prompt_window(&app_for_main);
                }
            });
        }
    });
}

fn pid_running(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn acquire_single_instance() -> bool {
    let path = lock_path();
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(pid) = text.trim().parse::<u32>() {
            if pid != process::id() && pid_running(pid) {
                return false;
            }
        }
    }
    let _ = fs::create_dir_all(
        path.parent()
            .unwrap_or_else(|| std::path::Path::new("/tmp")),
    );
    fs::write(path, process::id().to_string()).is_ok()
}

fn parse_shortcut(hotkey: &str) -> Option<Shortcut> {
    let (mods, key) = hotkeys::hotkey_parts(hotkey);
    let mut modifiers = Modifiers::empty();
    if mods.contains("ctrl") {
        modifiers |= Modifiers::CONTROL;
    }
    if mods.contains("alt") {
        modifiers |= Modifiers::ALT;
    }
    if mods.contains("shift") {
        modifiers |= Modifiers::SHIFT;
    }
    if mods.contains("super") {
        modifiers |= Modifiers::SUPER;
    }
    let code = match key.as_str() {
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        _ => return None,
    };
    let modifiers = if modifiers.is_empty() {
        None
    } else {
        Some(modifiers)
    };
    Some(Shortcut::new(modifiers, code))
}

pub fn run() {
    if std::env::var_os("DISPLAY").is_some() {
        // GNOME Wayland forbids normal apps from freely positioning windows.
        // This panel needs cursor-following prompt/feedback overlays, so use
        // XWayland when available. Windows are created on demand and destroyed
        // on close, so this no longer leaves hidden laggy X11/WebKit windows.
        std::env::set_var("GDK_BACKEND", "x11");
    }
    let command_on_startup = startup_command();
    if !acquire_single_instance() {
        let _ = send_request_to_running_instance(command_on_startup.unwrap_or("show_dropdown"));
        return;
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RuntimeState(Mutex::new(state::load_state())))
        .setup(move |app| {
            tray::install(app)?;
            start_ipc_server(app.handle());
            register_prompt_shortcut(app.handle());
            commands::sync_gnome_prompt_shortcut(&config::load_config().prompt_hotkey);
            context::start_browser_bridge();
            let _ = integrations::vscode::install();
            let _ = integrations::obsidian::install();
            let _ = integrations::browser::install();
            if let Some(command) = command_on_startup {
                let handle = app.handle().clone();
                let _ = app.handle().run_on_main_thread(move || match command {
                    "show_settings" | "settings" | "--settings" => {
                        let _ = crate::ui::windows::show_settings(handle.clone());
                    }
                    "show_dropdown" | "dropdown" | "--dropdown" | "open" | "--open" => {
                        let _ = crate::ui::windows::show_dropdown(handle.clone());
                    }
                    _ => {
                        let _ = crate::ui::windows::show_prompt_window(&handle);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::save_settings,
            commands::available_terminals,
            commands::submit_prompt,
            commands::submit_prompt_async,
            commands::switch_session,
            commands::start_new_section,
            commands::normalize_prompt_text,
            commands::capture_screenshot,
            commands::active_context_snapshot,
            commands::popup_context_chips,
            commands::diagnostics_report,
            commands::launch_terminal,
            commands::integration_status,
            commands::refresh_integrations,
            crate::ui::windows::show_prompt,
            crate::ui::windows::show_dropdown,
            crate::ui::windows::show_settings,
            crate::ui::windows::hide_prompt,
            crate::ui::windows::prompt_follow_mouse_tick,
            crate::ui::windows::show_feedback,
            crate::ui::windows::current_feedback,
            crate::ui::windows::hide_feedback,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build jcode-panel tauri app");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            api.prevent_exit();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_hotkey_support_rejects_mouse_buttons() {
        assert!(prompt_hotkey_supported("Super+Z"));
        assert!(prompt_hotkey_supported("F8"));
        assert!(!prompt_hotkey_supported("Mouse9"));
        assert!(!prompt_hotkey_supported("Super+Mouse8"));
    }

    #[test]
    fn invalid_prompt_hotkey_falls_back_to_f8() {
        assert_eq!(
            parse_prompt_shortcut_or_default("Mouse9"),
            parse_shortcut("F8").unwrap()
        );
    }
}
