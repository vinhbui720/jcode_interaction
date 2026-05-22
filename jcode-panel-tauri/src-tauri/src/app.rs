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
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process,
    sync::Mutex,
    thread,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn register_prompt_shortcut(app: &tauri::AppHandle) {
    let cfg = config::load_config();
    let shortcut =
        parse_shortcut(&cfg.prompt_hotkey).unwrap_or_else(|| Shortcut::new(None, Code::F8));
    let handle = app.clone();
    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let _ = crate::ui::windows::show_prompt_window(&handle);
            }
        });
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

fn send_prompt_request_to_running_instance() -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path()) else {
        return false;
    };
    stream.write_all(b"show_prompt\n").is_ok()
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
            let Ok(_stream) = stream else {
                continue;
            };
            let app_for_main = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = crate::ui::windows::show_prompt_window(&app_for_main);
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
    std::env::set_var("GDK_BACKEND", "x11");
    let show_prompt_on_startup = cli_wants_prompt();
    if !acquire_single_instance() {
        if show_prompt_on_startup {
            let _ = send_prompt_request_to_running_instance();
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RuntimeState(Mutex::new(state::load_state())))
        .setup(move |app| {
            tray::install(app)?;
            start_ipc_server(app.handle());
            register_prompt_shortcut(app.handle());
            context::start_browser_bridge();
            let _ = integrations::vscode::install();
            let _ = integrations::obsidian::install();
            if show_prompt_on_startup {
                let handle = app.handle().clone();
                let _ = app.handle().run_on_main_thread(move || {
                    let _ = crate::ui::windows::show_prompt_window(&handle);
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
        .run(tauri::generate_context!())
        .expect("failed to run jcode-panel tauri app");
}
