use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::models::AppError;

const WARP_API_BASE: &str = "https://api.cloudflareclient.com/v0a2158/reg";
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

pub fn save_warp_config<R: Runtime>(
    app: &AppHandle<R>,
    config: &WarpConfig,
) -> Result<(), AppError> {
    let path = warp_config_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(config)?;
    fs::write(&path, &data)?;
    Ok(())
}

/// Load existing WARP config or register a new one.
pub fn load_or_register<R: Runtime>(app: &AppHandle<R>) -> Result<WarpConfig, AppError> {
    if let Some(config) = load_warp_config(app) {
        log::info!(
            "Loaded existing WARP config (device_id={})",
            config.device_id
        );
        return Ok(config);
    }

    log::info!("No WARP config found, registering new device...");
    let config = register_warp()?;
    save_warp_config(app, &config)?;
    log::info!(
        "WARP registered: device_id={}, endpoint={}",
        config.device_id,
        config.endpoint
    );
    Ok(config)
}

fn warp_config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, AppError> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("Failed to get app config dir: {e}")))?;
    Ok(config_dir.join(WARP_CONFIG_FILE))
}

/// Register a new device with the Cloudflare WARP API and return a WarpConfig.
fn register_warp() -> Result<WarpConfig, AppError> {
    // Generate x25519 keypair
    let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let public = PublicKey::from(&secret);
    let private_key_b64 = BASE64.encode(secret.as_bytes());
    let public_key_b64 = BASE64.encode(public.as_bytes());

    // Register with WARP API
    let reg_body = serde_json::json!({
        "key": public_key_b64,
        "install_id": "",
        "fcm_token": "",
        "tos": "2024-01-01T00:00:00+00:00",
        "model": "Android",
        "type": "Android",
        "locale": "en_US"
    });

    let resp: serde_json::Value = ureq::post(WARP_API_BASE)
        .set("Content-Type", "application/json")
        .send_json(&reg_body)
        .map_err(|e| AppError::Config(format!("WARP registration failed: {e}")))?
        .into_json()
        .map_err(|e| AppError::Config(format!("WARP registration parse error: {e}")))?;

    let device_id = resp["id"]
        .as_str()
        .ok_or_else(|| AppError::Config("WARP response missing 'id'".to_string()))?
        .to_string();
    let access_token = resp["token"]
        .as_str()
        .ok_or_else(|| AppError::Config("WARP response missing 'token'".to_string()))?
        .to_string();

    // Fetch full device config
    let config_url = format!("{}/{}", WARP_API_BASE, device_id);
    let cfg_resp: serde_json::Value = ureq::get(&config_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .call()
        .map_err(|e| AppError::Config(format!("WARP config fetch failed: {e}")))?
        .into_json()
        .map_err(|e| AppError::Config(format!("WARP config parse error: {e}")))?;

    let config = &cfg_resp["config"];
    let interface = &config["interface"];
    let peer = &config["peers"][0];

    let address_v4 = interface["addresses"]["v4"]
        .as_str()
        .unwrap_or("172.16.0.2")
        .to_string();
    let address_v6 = interface["addresses"]["v6"]
        .as_str()
        .unwrap_or("fd01:db8:1111::2")
        .to_string();
    let peer_public_key = peer["public_key"]
        .as_str()
        .unwrap_or("bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=")
        .to_string();

    // Endpoint: prefer v4
    let endpoint = peer["endpoint"]["v4"]
        .as_str()
        .or_else(|| peer["endpoint"]["host"].as_str())
        .unwrap_or("engage.cloudflareclient.com:2408")
        .to_string();

    // Reserved bytes from client_id
    let reserved = parse_client_id(&cfg_resp["client_id"]);

    Ok(WarpConfig {
        private_key: private_key_b64,
        address_v4,
        address_v6,
        peer_public_key,
        endpoint,
        reserved,
        device_id,
        access_token,
    })
}

/// Parse the client_id field into 3 reserved bytes.
/// client_id is a base64-encoded value; we take the first 3 bytes.
fn parse_client_id(value: &serde_json::Value) -> [u8; 3] {
    if let Some(s) = value.as_str() {
        if let Ok(bytes) = BASE64.decode(s) {
            if bytes.len() >= 3 {
                return [bytes[0], bytes[1], bytes[2]];
            }
        }
    }
    [0, 0, 0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_client_id_valid() {
        let val = serde_json::json!("AQID"); // base64 of [1, 2, 3]
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
