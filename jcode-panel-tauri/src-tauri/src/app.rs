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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(RuntimeState(Mutex::new(state::load_state())))
        .setup(|app| {
            tray::install(app)?;
            let _ = integrations::vscode::install();
            let _ = integrations::obsidian::install();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::save_settings,
            commands::submit_prompt,
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
