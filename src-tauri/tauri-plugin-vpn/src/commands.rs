use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Runtime};

use crate::VpnPluginExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnStatus {
    pub is_running: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VpnStats {
    pub upload: u64,
    pub download: u64,
}

#[command]
pub(crate) async fn start_vpn<R: Runtime>(
    app: AppHandle<R>,
    config_json: String,
    socks_port: u16,
) -> Result<(), String> {
    app.vpn()
        .start_vpn(config_json, socks_port)
        .map_err(|e| e.to_string())
}

#[command]
pub(crate) async fn stop_vpn<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    app.vpn().stop_vpn().map_err(|e| e.to_string())
}

#[command]
pub(crate) async fn get_vpn_status<R: Runtime>(app: AppHandle<R>) -> Result<VpnStatus, String> {
    app.vpn().get_status().map_err(|e| e.to_string())
}

#[command]
pub(crate) async fn query_stats<R: Runtime>(app: AppHandle<R>) -> Result<VpnStats, String> {
    app.vpn().query_stats().map_err(|e| e.to_string())
}
