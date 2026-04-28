pub mod commands;
pub mod config;
pub mod models;
#[cfg(desktop)]
pub mod network;
#[cfg(desktop)]
pub mod proxy;
pub mod storage;
#[cfg(desktop)]
pub mod tray;
#[cfg(target_os = "linux")]
pub mod tun;
pub mod uri;
pub mod xray;

use tauri::Manager;
use xray::XrayManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_vpn::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_shell::init())?;

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Clean up stale TUN from previous crash (Linux only)
            #[cfg(target_os = "linux")]
            {
                let config_dir = app
                    .handle()
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
                tun::cleanup_stale_tun(&config_dir);
            }

            // Reset system proxy if it's still pointing at our ports from a prior
            // session that didn't shut down cleanly. Must happen BEFORE auto-connect
            // (which would set it again) — otherwise apps would try to reach a dead
            // SOCKS proxy during the window between app start and VPN connect.
            #[cfg(desktop)]
            proxy::reset_stale_system_proxy();

            app.manage(XrayManager::new());

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

            let settings = storage::load_settings(&handle).unwrap_or_default();

            // On Android, the VPN foreground service can outlive the activity
            // (swipe from recents). If it's still running when we start up,
            // adopt its state so the UI shows Connected instead of Disconnected,
            // and skip the auto-connect path.
            #[cfg(mobile)]
            let vpn_already_running = {
                use tauri_plugin_vpn::VpnPluginExt;
                match handle.vpn().get_status() {
                    Ok(status) if status.is_running => {
                        if let Some(ref server_id) = settings.last_server_id {
                            if let Ok(servers) = storage::load_servers(&handle) {
                                if let Some(server) = servers.iter().find(|s| s.id == *server_id) {
                                    app.state::<XrayManager>().adopt_running_state(server);
                                    log::info!(
                                        "Adopted running VPN session for server {}",
                                        server.name
                                    );
                                }
                            }
                        }
                        true
                    }
                    _ => false,
                }
            };
            #[cfg(desktop)]
            let vpn_already_running = false;

            // Auto-connect on startup
            if !vpn_already_running && settings.auto_connect {
                if let Some(ref server_id) = settings.last_server_id {
                    if let Ok(servers) = storage::load_servers(&handle) {
                        if let Some(server) = servers.iter().find(|s| s.id == *server_id) {
                            let manager = app.state::<XrayManager>();
                            if let Err(e) = manager.start(&handle, server, &settings.bypass_domains)
                            {
                                log::warn!("Auto-connect failed: {e}");
                            }
                        }
                    }
                }
            }

            Ok(())
        })
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
            commands::apply_bypass_domains,
            uri::parse_vless_uri_cmd,
            uri::export_vless_uri,
            commands::detect_vpn_interfaces,
            commands::is_battery_optimization_ignored,
            commands::request_ignore_battery_optimization,
            commands::open_oem_background_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
