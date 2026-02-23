use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let open_item = MenuItem::with_id(app, "open", "Open PrintQueue", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause", "Pause Watching", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&open_item, &pause_item, &quit_item])?;

    let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
        Image::from_bytes(include_bytes!("../icons/32x32.png")).expect("failed to load tray icon")
    });

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip("PrintQueue")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "pause" => {
                // Toggle watcher
                use std::sync::Arc;
                use crate::watcher::{WatcherState, WatcherStatus};
                if let Some(ws) = app.try_state::<Arc<WatcherState>>() {
                    if let Ok(status) = ws.status.lock() {
                        if *status == WatcherStatus::Active {
                            drop(status);
                            crate::watcher::stop_watcher_inner(&ws);
                        }
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
