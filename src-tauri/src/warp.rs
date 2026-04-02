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
const WARP_LOG_FILE: &str = "warp_log.txt";

/// Append a timestamped line to warp_log.txt for debugging.
/// Writes to both config_dir and /sdcard/ (readable via adb pull).
fn warp_log(config_dir: &std::path::Path, msg: &str) {
    use std::io::Write;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let line = format!("[{ts}] {msg}\n");

    // Write to app config dir
    let path = config_dir.join(WARP_LOG_FILE);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }

    // Also write to /sdcard/ so adb pull can read it without root
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/sdcard/warp_log.txt")
    {
        let _ = f.write_all(line.as_bytes());
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

/// Ensure WARP credentials exist — register in background if missing.
/// Call this at app startup on mobile. Non-blocking.
pub fn ensure_registered<R: Runtime>(app: &AppHandle<R>) {
    // Always write a breadcrumb so we know this function was called
    let sdcard = std::path::PathBuf::from("/sdcard");
    warp_log(&sdcard, "ensure_registered called");

    let config_dir = match app.path().app_config_dir() {
        Ok(d) => {
            warp_log(&sdcard, &format!("config_dir: {}", d.display()));
            d
        }
        Err(e) => {
            warp_log(&sdcard, &format!("ERROR: app_config_dir failed: {e}"));
            return;
        }
    };

    if load_warp_config(app).is_some() {
        warp_log(&sdcard, "warp.json already exists, skipping registration");
        return;
    }

    let path = config_dir.join(WARP_CONFIG_FILE);

    let log_dir = config_dir.clone();
    std::thread::spawn(move || {
        warp_log(&log_dir, "Starting background registration...");
        match register_warp(&log_dir) {
            Ok(config) => {
                let _ = std::fs::create_dir_all(path.parent().unwrap());
                match serde_json::to_string_pretty(&config) {
                    Ok(data) => {
                        let _ = std::fs::write(&path, data);
                        warp_log(
                            &log_dir,
                            &format!(
                                "OK: device_id={}, endpoint={}, addr={}",
                                config.device_id, config.endpoint, config.address_v4
                            ),
                        );
                    }
                    Err(e) => warp_log(&log_dir, &format!("ERROR serializing config: {e}")),
                }
            }
            Err(e) => {
                warp_log(&log_dir, &format!("ERROR registration failed: {e}"));
            }
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

/// Register a new device with the Cloudflare WARP API and return a WarpConfig.
fn register_warp(log_dir: &std::path::Path) -> Result<WarpConfig, AppError> {
    // Generate x25519 keypair
    let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let public = PublicKey::from(&secret);
    let private_key_b64 = BASE64.encode(secret.as_bytes());
    let public_key_b64 = BASE64.encode(public.as_bytes());

    warp_log(
        log_dir,
        &format!("Generated keypair, pubkey={public_key_b64}"),
    );

    // Register with WARP API
    let reg_body = serde_json::json!({
        "key": public_key_b64,
        "install_id": "",
        "fcm_token": "",
        "tos": "2024-01-01T00:00:00+00:00",
        "model": "PC",
        "type": "Android",
        "locale": "en_US"
    });

    warp_log(log_dir, &format!("POST {WARP_API_BASE}"));

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build();
    let resp: serde_json::Value = agent
        .post(WARP_API_BASE)
        .set("Content-Type", "application/json")
        .set("CF-Client-Version", "a-7.21-0721")
        .send_json(&reg_body)
        .map_err(|e| {
            warp_log(log_dir, &format!("POST failed: {e}"));
            AppError::Config(format!("WARP registration failed: {e}"))
        })?
        .into_json()
        .map_err(|e| {
            warp_log(log_dir, &format!("JSON parse failed: {e}"));
            AppError::Config(format!("WARP registration parse error: {e}"))
        })?;

    warp_log(
        log_dir,
        &format!(
            "Response keys: {:?}",
            resp.as_object().map(|o| o.keys().collect::<Vec<_>>())
        ),
    );

    let device_id = resp["id"]
        .as_str()
        .ok_or_else(|| {
            warp_log(log_dir, &format!("Missing 'id' in response: {resp}"));
            AppError::Config("WARP response missing 'id'".to_string())
        })?
        .to_string();
    let access_token = resp["token"]
        .as_str()
        .ok_or_else(|| {
            warp_log(log_dir, "Missing 'token' in response");
            AppError::Config("WARP response missing 'token'".to_string())
        })?
        .to_string();

    // The registration response already contains the full config
    let config = &resp["config"];
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

    // Endpoint: prefer host (has correct port), v4/v6 may have port=0
    let endpoint = peer["endpoint"]["host"]
        .as_str()
        .unwrap_or("engage.cloudflareclient.com:2408")
        .to_string();

    // Reserved bytes from client_id (in config section)
    let reserved = parse_client_id(&config["client_id"]);

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
/// client_id is base64-encoded; decode and take first 3 bytes.
fn parse_client_id(value: &serde_json::Value) -> [u8; 3] {
    if let Some(s) = value.as_str() {
        // Try base64 first (may need padding)
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
        // base64 of [0xe7, 0x02, 0xeb] = "5wLr"
        let val = serde_json::json!("5wLr");
        assert_eq!(parse_client_id(&val), [0xe7, 0x02, 0xeb]);
    }

    #[test]
    fn parse_client_id_padded() {
        // "AQID" = base64 of [1, 2, 3]
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
