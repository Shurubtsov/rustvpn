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
    fs::write(&path, data)?;
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
    let settings: AppSettings = serde_json::from_str(&data)?;
    Ok(settings)
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
    fs::write(&path, data)?;
    Ok(())
}
