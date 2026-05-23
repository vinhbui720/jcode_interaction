use crate::ui::{status, windows};
use std::{path::PathBuf, process::Command};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

pub fn install(app: &mut App) -> tauri::Result<()> {
    let open = MenuItem::with_id(
        app,
        "dropdown",
        "Open Jcode Interaction",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let builder = TrayIconBuilder::with_id("jcode-panel")
        .tooltip("Jcode Interaction")
        .title("jcode")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "dropdown" => {
                let _ = windows::show_dropdown(app.clone());
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
                let _ = windows::show_dropdown(app.clone());
            }
        });

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
        // Match the Python app title/body, but use the installed themed icon so
        // GNOME notifications show the same panel mark instead of a generic app.
        .arg("-i")
        .arg(notification_icon_name_or_path(app))
        .arg("-u")
        .arg("normal")
        .arg("-t")
        .arg("5000");
    let _ = command
        .arg("jcode-panel is running")
        .arg("Use the top-bar icon, jcode-panel, or jcp to open it.")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn notification_icon_name_or_path(app: &App) -> String {
    themed_icon_path()
        .filter(|path| path.exists())
        .or_else(|| notification_icon_path(app))
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "jcode-panel".into())
}

fn notification_icon_path(_app: &App) -> Option<PathBuf> {
    icon_path_candidates(_app)
        .into_iter()
        .find(|path| path.exists())
}

fn icon_path_candidates(app: &App) -> Vec<PathBuf> {
    [
        themed_icon_path(),
        std::env::current_dir()
            .ok()
            .map(|dir| dir.join("src-tauri/icons/128x128.png")),
        std::env::current_dir()
            .ok()
            .map(|dir| dir.join("src-tauri/icons/32x32.png")),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("icons/128x128.png"))),
        app.path()
            .resource_dir()
            .ok()
            .map(|dir| dir.join("icons/128x128.png")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn themed_icon_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".local")
            .join("share")
            .join("icons")
            .join("hicolor")
            .join("scalable")
            .join("apps")
            .join("jcode-panel.svg")
    })
}
