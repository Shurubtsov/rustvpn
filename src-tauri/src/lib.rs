pub mod commands;
pub mod config;
pub mod models;
pub mod proxy;
pub mod storage;
#[cfg(desktop)]
pub mod tray;
pub mod uri;
pub mod xray;

use tauri::Manager;
use xray::XrayManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(XrayManager::new())
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::get_status,
            commands::get_connection_info,
            commands::test_connection,
            commands::get_socks_port,
            commands::validate_config,
            commands::get_servers,
            commands::add_server,
            commands::update_server,
            commands::delete_server,
            commands::export_servers,
            commands::import_servers,
            commands::get_speed_stats,
            commands::get_logs,
            commands::clear_logs,
            commands::get_settings,
            commands::update_settings,
            uri::parse_vless_uri_cmd,
            uri::export_vless_uri,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let handle = app.handle().clone();

            // Setup system tray (desktop only — mobile has no tray)
            #[cfg(desktop)]
            {
                tray::setup_tray(&handle)?;

                // Hide to tray instead of closing
                let window = app.get_webview_window("main").unwrap();
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            // Auto-connect on startup
            let settings = storage::load_settings(&handle).unwrap_or_default();
            if settings.auto_connect {
                if let Some(ref server_id) = settings.last_server_id {
                    if let Ok(servers) = storage::load_servers(&handle) {
                        if let Some(server) = servers.iter().find(|s| s.id == *server_id) {
                            let manager = app.state::<XrayManager>();
                            if let Err(e) = manager.start(&handle, server) {
                                log::warn!("Auto-connect failed: {e}");
                            }
                        }
                    }
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
