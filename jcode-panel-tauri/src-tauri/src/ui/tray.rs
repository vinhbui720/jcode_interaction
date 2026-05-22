use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

pub fn install(app: &mut App) -> tauri::Result<()> {
    let prompt = MenuItem::with_id(app, "prompt", "Prompt", true, None::<&str>)?;
    let dropdown = MenuItem::with_id(app, "dropdown", "Open Panel", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&prompt, &dropdown, &settings, &quit])?;

    TrayIconBuilder::with_id("jcode-panel")
        .tooltip("Jcode Interaction")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "prompt" => {
                let _ = app.get_webview_window("prompt").map(|w| {
                    let _ = w.show();
                    let _ = w.set_focus();
                });
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
                let _ = app.get_webview_window("prompt").map(|w| {
                    let _ = w.show();
                    let _ = w.set_focus();
                });
            }
        })
        .build(app)?;
    Ok(())
}
