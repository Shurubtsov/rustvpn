use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

use crate::models::{AppError, AppSettings, ServerConfig};

const SERVERS_FILE: &str = "servers.json";
const SETTINGS_FILE: &str = "settings.json";

fn servers_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, AppError> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("Failed to get app config dir: {e}")))?;
    Ok(config_dir.join(SERVERS_FILE))
}

pub fn load_servers<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<ServerConfig>, AppError> {
    let path = servers_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    let servers: Vec<ServerConfig> = serde_json::from_str(&data)?;
    Ok(servers)
}

pub fn save_servers<R: Runtime>(
    app: &AppHandle<R>,
    servers: &[ServerConfig],
) -> Result<(), AppError> {
    let path = servers_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(servers)?;
    fs::write(&path, &data)?;
    set_restrictive_permissions(&path);
    Ok(())
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, AppError> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("Failed to get app config dir: {e}")))?;
    Ok(config_dir.join(SETTINGS_FILE))
}

pub fn load_settings<R: Runtime>(app: &AppHandle<R>) -> Result<AppSettings, AppError> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let data = fs::read_to_string(&path)?;
    match serde_json::from_str::<AppSettings>(&data) {
        Ok(mut settings) => {
            // Migrate: bypass_domains was added later; if saved as empty (e.g. from an older
            // version that stored the field as []), restore defaults so corporate domains work.
            if settings.bypass_domains.is_empty() {
                settings.bypass_domains = AppSettings::default().bypass_domains;
            }
            Ok(settings)
        }
        Err(e) => {
            log::warn!("Corrupted settings.json, resetting to defaults: {e}");
            let _ = fs::remove_file(&path);
            Ok(AppSettings::default())
        }
    }
}

pub fn save_settings<R: Runtime>(
    app: &AppHandle<R>,
    settings: &AppSettings,
) -> Result<(), AppError> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(settings)?;
    fs::write(&path, &data)?;
    set_restrictive_permissions(&path);
    Ok(())
}

/// Set file permissions to 0o600 (owner read/write only) on Unix.
#[cfg(unix)]
fn set_restrictive_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_restrictive_permissions(_path: &std::path::Path) {}
