use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::models::{AppError, LogEntry};

const WARP_CONFIG_FILE: &str = "warp.json";

/// Push a log entry into the app's log buffer (visible on the Logs page).
fn push_log(logs: &Arc<Mutex<VecDeque<LogEntry>>>, level: &str, msg: &str) {
    let entry = LogEntry {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        level: level.to_string(),
        message: msg.to_string(),
    };
    if let Ok(mut buffer) = logs.lock() {
        if buffer.len() >= 1000 {
            buffer.pop_front();
        }
        buffer.push_back(entry);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpConfig {
    pub private_key: String,
    pub address_v4: String,
    pub address_v6: String,
    pub peer_public_key: String,
    pub endpoint: String,
    pub reserved: [u8; 3],
    pub device_id: String,
    pub access_token: String,
}

pub fn load_warp_config<R: Runtime>(app: &AppHandle<R>) -> Option<WarpConfig> {
    let path = warp_config_path(app).ok()?;
    if !path.exists() {
        return None;
    }
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Register WARP in the background via the Android plugin's native HTTP.
/// Only call on WiFi — cellular may block the Cloudflare API.
pub fn register_in_background<R: Runtime + 'static>(
    app: &AppHandle<R>,
    logs: &Arc<Mutex<VecDeque<LogEntry>>>,
) {
    if load_warp_config(app).is_some() {
        push_log(logs, "info", "[warp] Already registered, skipping");
        return;
    }

    let config_dir = match app.path().app_config_dir() {
        Ok(d) => d,
        Err(e) => {
            push_log(logs, "error", &format!("[warp] app_config_dir failed: {e}"));
            return;
        }
    };

    // Generate x25519 keypair
    let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let public = PublicKey::from(&secret);
    let private_key_b64 = BASE64.encode(secret.as_bytes());
    let public_key_b64 = BASE64.encode(public.as_bytes());

    let path = config_dir.join(WARP_CONFIG_FILE);
    let logs_ref = logs.clone();
    let app_handle = app.clone();

    std::thread::spawn(move || {
        push_log(
            &logs_ref,
            "info",
            "[warp] Registering with Cloudflare via Android HTTP...",
        );

        // Call Kotlin plugin to do the HTTP POST (uses Android's native TLS)
        use tauri_plugin_vpn::VpnPluginExt;
        let resp = match app_handle.vpn().register_warp(&public_key_b64) {
            Ok(v) => v,
            Err(e) => {
                push_log(
                    &logs_ref,
                    "error",
                    &format!("[warp] Registration failed: {e}"),
                );
                return;
            }
        };

        // Parse response
        let device_id = match resp["id"].as_str() {
            Some(id) => id.to_string(),
            None => {
                push_log(
                    &logs_ref,
                    "error",
                    &format!("[warp] Response missing 'id': {resp}"),
                );
                return;
            }
        };
        let access_token = resp["token"].as_str().unwrap_or_default().to_string();

        let config_section = &resp["config"];
        let interface = &config_section["interface"];
        let peer = &config_section["peers"][0];

        let warp_config = WarpConfig {
            private_key: private_key_b64,
            address_v4: interface["addresses"]["v4"]
                .as_str()
                .unwrap_or("172.16.0.2")
                .to_string(),
            address_v6: interface["addresses"]["v6"]
                .as_str()
                .unwrap_or("fd01:db8:1111::2")
                .to_string(),
            peer_public_key: peer["public_key"]
                .as_str()
                .unwrap_or("bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=")
                .to_string(),
            endpoint: peer["endpoint"]["host"]
                .as_str()
                .unwrap_or("engage.cloudflareclient.com:2408")
                .to_string(),
            reserved: parse_client_id(&config_section["client_id"]),
            device_id,
            access_token,
        };

        // Save to disk
        let _ = fs::create_dir_all(path.parent().unwrap());
        match serde_json::to_string_pretty(&warp_config) {
            Ok(data) => {
                let _ = fs::write(&path, &data);
                push_log(
                    &logs_ref,
                    "info",
                    &format!(
                        "[warp] Registered OK: endpoint={}, addr={}",
                        warp_config.endpoint, warp_config.address_v4
                    ),
                );
            }
            Err(e) => push_log(
                &logs_ref,
                "error",
                &format!("[warp] Failed to save config: {e}"),
            ),
        }
    });
}

fn warp_config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, AppError> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("Failed to get app config dir: {e}")))?;
    Ok(config_dir.join(WARP_CONFIG_FILE))
}

fn parse_client_id(value: &serde_json::Value) -> [u8; 3] {
    if let Some(s) = value.as_str() {
        let padded = match s.len() % 4 {
            2 => format!("{s}=="),
            3 => format!("{s}="),
            _ => s.to_string(),
        };
        if let Ok(bytes) = BASE64.decode(&padded) {
            let mut result = [0u8; 3];
            for (i, &b) in bytes.iter().take(3).enumerate() {
                result[i] = b;
            }
            return result;
        }
    }
    [0, 0, 0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_client_id_base64() {
        let val = serde_json::json!("5wLr");
        assert_eq!(parse_client_id(&val), [0xe7, 0x02, 0xeb]);
    }

    #[test]
    fn parse_client_id_padded() {
        let val = serde_json::json!("AQID");
        assert_eq!(parse_client_id(&val), [1, 2, 3]);
    }

    #[test]
    fn parse_client_id_null() {
        let val = serde_json::json!(null);
        assert_eq!(parse_client_id(&val), [0, 0, 0]);
    }

    #[test]
    fn parse_client_id_empty() {
        let val = serde_json::json!("");
        assert_eq!(parse_client_id(&val), [0, 0, 0]);
    }
}
