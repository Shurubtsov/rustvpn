use tauri::{AppHandle, Runtime, State};

use crate::models::{
    AppSettings, ConnectionInfo, ConnectionStatus, LogEntry, ServerConfig, SpeedStats,
};
use crate::network::{self, DetectedVpn};
use crate::storage;
use crate::xray::XrayManager;

#[tauri::command]
pub fn connect<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, XrayManager>,
    server_config: ServerConfig,
) -> Result<(), String> {
    server_config.validate()?;
    let settings = storage::load_settings(&app).unwrap_or_default();
    manager
        .start(&app, &server_config, &settings.bypass_domains)
        .map_err(|e| e.to_string())?;

    // Save last server id for auto-connect and tray reconnect
    let mut settings = settings;
    settings.last_server_id = Some(server_config.id.clone());
    let _ = storage::save_settings(&app, &settings);

    Ok(())
}

#[tauri::command]
pub fn validate_config(server_config: ServerConfig) -> Result<(), String> {
    server_config.validate()
}

#[tauri::command]
pub fn disconnect(manager: State<'_, XrayManager>) -> Result<(), String> {
    manager.stop().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_status(manager: State<'_, XrayManager>) -> Result<ConnectionStatus, String> {
    Ok(manager.status().status)
}

#[tauri::command]
pub fn get_connection_info(manager: State<'_, XrayManager>) -> Result<ConnectionInfo, String> {
    Ok(manager.status())
}

#[tauri::command]
pub fn test_connection(manager: State<'_, XrayManager>) -> Result<bool, String> {
    manager.test_connection().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_socks_port(manager: State<'_, XrayManager>) -> Result<u16, String> {
    Ok(manager.socks_port())
}

#[tauri::command]
pub fn get_servers<R: Runtime>(app: AppHandle<R>) -> Result<Vec<ServerConfig>, String> {
    storage::load_servers(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_server<R: Runtime>(
    app: AppHandle<R>,
    server_config: ServerConfig,
) -> Result<ServerConfig, String> {
    let mut servers = storage::load_servers(&app).map_err(|e| e.to_string())?;
    let mut new_server = server_config;
    // Always assign a fresh id
    new_server.id = uuid::Uuid::new_v4().to_string();
    servers.push(new_server.clone());
    storage::save_servers(&app, &servers).map_err(|e| e.to_string())?;
    Ok(new_server)
}

#[tauri::command]
pub fn update_server<R: Runtime>(
    app: AppHandle<R>,
    server_config: ServerConfig,
) -> Result<(), String> {
    let mut servers = storage::load_servers(&app).map_err(|e| e.to_string())?;
    let pos = servers
        .iter()
        .position(|s| s.id == server_config.id)
        .ok_or_else(|| format!("Server with id {} not found", server_config.id))?;
    servers[pos] = server_config;
    storage::save_servers(&app, &servers).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_server<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let mut servers = storage::load_servers(&app).map_err(|e| e.to_string())?;
    let len_before = servers.len();
    servers.retain(|s| s.id != id);
    if servers.len() == len_before {
        return Err(format!("Server with id {id} not found"));
    }
    storage::save_servers(&app, &servers).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_servers<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    let servers = storage::load_servers(&app).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&servers).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_servers<R: Runtime>(
    app: AppHandle<R>,
    json: String,
) -> Result<Vec<ServerConfig>, String> {
    let imported: Vec<ServerConfig> =
        serde_json::from_str(&json).map_err(|e| format!("Invalid JSON: {e}"))?;
    let mut servers = storage::load_servers(&app).map_err(|e| e.to_string())?;
    // Assign fresh ids to imported servers to avoid collisions
    let new_servers: Vec<ServerConfig> = imported
        .into_iter()
        .map(|mut s| {
            s.id = uuid::Uuid::new_v4().to_string();
            s
        })
        .collect();
    servers.extend(new_servers.clone());
    storage::save_servers(&app, &servers).map_err(|e| e.to_string())?;
    Ok(new_servers)
}

#[tauri::command]
pub async fn get_speed_stats<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, XrayManager>,
) -> Result<SpeedStats, String> {
    manager.query_stats(&app).await.map_err(|e| e.to_string())
}

// Logs
#[tauri::command]
pub fn get_logs(manager: State<'_, XrayManager>) -> Result<Vec<LogEntry>, String> {
    Ok(manager.get_logs())
}

#[tauri::command]
pub fn clear_logs(manager: State<'_, XrayManager>) -> Result<(), String> {
    manager.clear_logs();
    Ok(())
}

// Settings
#[tauri::command]
pub fn get_settings<R: Runtime>(app: AppHandle<R>) -> Result<AppSettings, String> {
    storage::load_settings(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_settings<R: Runtime>(app: AppHandle<R>, settings: AppSettings) -> Result<(), String> {
    storage::save_settings(&app, &settings).map_err(|e| e.to_string())
}

// VPN detection
#[tauri::command]
pub fn detect_vpn_interfaces() -> Result<Vec<DetectedVpn>, String> {
    Ok(network::detect_vpn_routes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RealitySettings;

    fn valid_config() -> ServerConfig {
        ServerConfig {
            id: "test-id-1234".to_string(),
            name: "Test".to_string(),
            address: "1.2.3.4".to_string(),
            port: 443,
            uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings {
                public_key: "abc123".to_string(),
                short_id: "def456".to_string(),
                server_name: "example.com".to_string(),
                fingerprint: "chrome".to_string(),
            },
        }
    }

    #[test]
    fn validate_config_accepts_valid_config() {
        let config = valid_config();
        assert!(validate_config(config).is_ok());
    }

    #[test]
    fn validate_config_rejects_empty_address() {
        let mut config = valid_config();
        config.address = String::new();
        let err = validate_config(config).unwrap_err();
        assert!(
            err.contains("address"),
            "expected 'address' in error: {err}"
        );
    }

    #[test]
    fn validate_config_rejects_port_zero() {
        let mut config = valid_config();
        config.port = 0;
        let err = validate_config(config).unwrap_err();
        assert!(err.contains("port"), "expected 'port' in error: {err}");
    }

    #[test]
    fn validate_config_rejects_invalid_uuid() {
        let mut config = valid_config();
        config.uuid = "not-a-valid-uuid".to_string();
        let err = validate_config(config).unwrap_err();
        assert!(err.contains("UUID"), "expected 'UUID' in error: {err}");
    }
}
