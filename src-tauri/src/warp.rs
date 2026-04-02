use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::models::AppError;

const WARP_API_BASE: &str = "https://api.cloudflareclient.com/v0a884/reg";
const WARP_CONFIG_FILE: &str = "warp.json";

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


/// Register WARP in the background — only call this when on WiFi.
/// On cellular, the Cloudflare API may be unreachable (blocked/slow).
pub fn register_in_background<R: Runtime>(app: &AppHandle<R>) {
    if load_warp_config(app).is_some() {
        log::info!("[warp] Already registered, skipping");
        return;
    }

    let config_dir = match app.path().app_config_dir() {
        Ok(d) => d,
        Err(e) => {
            log::error!("[warp] app_config_dir failed: {e}");
            return;
        }
    };
    let path = config_dir.join(WARP_CONFIG_FILE);

    std::thread::spawn(move || {
        log::info!("[warp] Registering with Cloudflare WARP API...");
        match register_warp() {
            Ok(config) => {
                let _ = fs::create_dir_all(path.parent().unwrap());
                match serde_json::to_string_pretty(&config) {
                    Ok(data) => {
                        let _ = fs::write(&path, data);
                        log::info!(
                            "[warp] Registered: device_id={}, endpoint={}, addr={}",
                            config.device_id,
                            config.endpoint,
                            config.address_v4
                        );
                    }
                    Err(e) => log::error!("[warp] Failed to save config: {e}"),
                }
            }
            Err(e) => log::error!("[warp] Registration failed: {e}"),
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

/// Register a new device with the Cloudflare WARP API.
fn register_warp() -> Result<WarpConfig, AppError> {
    let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let public = PublicKey::from(&secret);
    let private_key_b64 = BASE64.encode(secret.as_bytes());
    let public_key_b64 = BASE64.encode(public.as_bytes());

    let reg_body = serde_json::json!({
        "key": public_key_b64,
        "install_id": "",
        "fcm_token": "",
        "tos": "2024-01-01T00:00:00+00:00",
        "model": "PC",
        "type": "Android",
        "locale": "en_US"
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build();
    let resp: serde_json::Value = agent
        .post(WARP_API_BASE)
        .set("Content-Type", "application/json")
        .set("CF-Client-Version", "a-7.21-0721")
        .send_json(&reg_body)
        .map_err(|e| AppError::Config(format!("WARP API POST failed: {e}")))?
        .into_json()
        .map_err(|e| AppError::Config(format!("WARP API response parse error: {e}")))?;

    let device_id = resp["id"]
        .as_str()
        .ok_or_else(|| AppError::Config(format!("WARP response missing 'id': {resp}")))?
        .to_string();
    let access_token = resp["token"]
        .as_str()
        .ok_or_else(|| AppError::Config("WARP response missing 'token'".to_string()))?
        .to_string();

    let config = &resp["config"];
    let interface = &config["interface"];
    let peer = &config["peers"][0];

    Ok(WarpConfig {
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
        reserved: parse_client_id(&config["client_id"]),
        device_id,
        access_token,
    })
}

/// Parse the client_id field into 3 reserved bytes.
/// client_id is base64-encoded; decode and take first 3 bytes.
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
