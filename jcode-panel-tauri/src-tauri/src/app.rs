use crate::{
    core::state,
    integrations,
    ui::{
        commands::{self, RuntimeState},
        tray,
    },
};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn register_prompt_shortcut(app: &tauri::AppHandle) {
    let shortcut = Shortcut::new(Some(Modifiers::empty()), Code::F8);
    let handle = app.clone();
    let _ = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(window) = handle.get_webview_window("prompt") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RuntimeState(Mutex::new(state::load_state())))
        .setup(|app| {
            tray::install(app)?;
            register_prompt_shortcut(app.handle());
            let _ = integrations::vscode::install();
            let _ = integrations::obsidian::install();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::save_settings,
            commands::submit_prompt,
            commands::switch_session,
            commands::start_new_section,
            commands::integration_status,
            commands::refresh_integrations,
            crate::ui::windows::show_prompt,
            crate::ui::windows::show_dropdown,
            crate::ui::windows::show_settings,
            crate::ui::windows::hide_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run jcode-panel tauri app");
}
