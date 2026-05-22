use crate::ui::{status, windows};
use std::{path::PathBuf, process::Command};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

pub fn install(app: &mut App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "dropdown", "Open", true, None::<&str>)?;
    let prompt = MenuItem::with_id(app, "prompt", "Prompt", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &prompt, &settings, &quit])?;

    let mut builder = TrayIconBuilder::with_id("jcode-panel")
        .tooltip("Jcode Interaction")
        .title("jcode")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "prompt" => {
                let _ = windows::show_prompt_window(app);
            }
            "dropdown" => {
                let _ = app.get_webview_window("dropdown").map(|w| {
                    let _ = w.show();
                    let _ = w.set_focus();
                });
            }
            "settings" => {
                let _ = app.get_webview_window("settings").map(|w| {
                    let _ = w.show();
                    let _ = w.set_focus();
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                let _ = windows::show_prompt_window(&app);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    let _ = status::refresh_header_status(app.handle());
    notify_startup(app);
    Ok(())
}

fn notify_startup(app: &App) {
    let mut command = Command::new("notify-send");
    command
        .arg("-a")
        .arg("jcode-panel")
        .arg("-u")
        .arg("normal")
        .arg("-t")
        .arg("5000");
    if let Some(icon) = notification_icon_path(app) {
        command.arg("-i").arg(icon);
    }
    let _ = command
        .arg("jcode-panel is running")
        .arg("Use the top-bar icon, jcode-panel, or jcp to open it.")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn notification_icon_path(_app: &App) -> Option<PathBuf> {
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|dir| dir.join("src-tauri/icons/128x128.png")),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("icons/128x128.png"))),
    ];
    candidates.into_iter().flatten().find(|path| path.exists())
}
