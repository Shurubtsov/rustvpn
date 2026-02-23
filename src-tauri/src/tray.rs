use log::warn;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Listener, Manager, Runtime};

use crate::models::ConnectionStatus;
use crate::storage;
use crate::xray::XrayManager;

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let toggle_item = MenuItem::with_id(app, "toggle_connection", "Connect", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &toggle_item, &quit_item])?;

    let toggle_clone = toggle_item.clone();
    app.listen("connection-status-changed", move |event| {
        let payload = event.payload();
        if payload.contains("connected") && !payload.contains("disconnected") {
            let _ = toggle_clone.set_text("Disconnect");
        } else {
            let _ = toggle_clone.set_text("Connect");
        }
    });

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("RustVPN")
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "toggle_connection" => {
                handle_toggle_connection(app);
            }
            "quit" => {
                let manager = app.state::<XrayManager>();
                let _ = manager.stop();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
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

fn handle_toggle_connection<R: Runtime>(app: &AppHandle<R>) {
    let manager = app.state::<XrayManager>();
    let info = manager.status();

    match info.status {
        ConnectionStatus::Connected | ConnectionStatus::Connecting => {
            let _ = manager.stop();
        }
        ConnectionStatus::Disconnected | ConnectionStatus::Error => {
            // Try to connect with last server
            let settings = storage::load_settings(app).unwrap_or_default();
            if let Some(ref server_id) = settings.last_server_id {
                if let Ok(servers) = storage::load_servers(app) {
                    if let Some(server) = servers.iter().find(|s| s.id == *server_id) {
                        if let Err(e) = manager.start(app, server) {
                            warn!("Tray connect failed: {e}");
                        }
                        return;
                    }
                }
            }
            // No last server — show the window instead
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        ConnectionStatus::Disconnecting => {
            // Do nothing while transitioning
        }
    }
}
