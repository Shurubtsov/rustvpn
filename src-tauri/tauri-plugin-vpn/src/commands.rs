use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Runtime};

use crate::VpnPluginExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnStatus {
    pub is_running: bool,
    pub last_error: Option<String>,
    #[serde(default)]
    pub xray_running: bool,
    #[serde(default)]
    pub hev_running: bool,
    #[serde(default)]
    pub tun_active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VpnStats {
    pub upload: u64,
    pub download: u64,
}

/// Result of a battery-optimization exemption request.
///
/// On Android the user has to actively grant the exemption from a system
/// settings screen; `granted` reflects the current `PowerManager` state after
/// the screen closes. On desktop this is always `true` since the concept does
/// not apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatteryOptResult {
    pub granted: bool,
}

/// Snapshot of whether the OS already considers the app exempt from battery
/// optimization. Used by the UI to decide whether to prompt the user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatteryOptStatus {
    pub ignored: bool,
}

/// Outcome of attempting to deep-link to the OEM-specific background-activity
/// settings page. `opened=false` means no candidate intent resolved; `fallback=true`
/// means we landed on the generic application-details screen instead of the
/// OEM-specific page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OemSettingsResult {
    pub opened: bool,
    #[serde(default)]
    pub fallback: bool,
}

#[command]
pub(crate) async fn start_vpn<R: Runtime>(
    app: AppHandle<R>,
    config_json: String,
    socks_port: u16,
    server_address: Option<String>,
) -> Result<(), String> {
    app.vpn()
        .start_vpn(config_json, socks_port, server_address.unwrap_or_default())
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

#[command]
pub(crate) async fn is_battery_optimization_ignored<R: Runtime>(
    app: AppHandle<R>,
) -> Result<BatteryOptStatus, String> {
    app.vpn()
        .is_battery_optimization_ignored()
        .map_err(|e| e.to_string())
}

#[command]
pub(crate) async fn request_ignore_battery_optimization<R: Runtime>(
    app: AppHandle<R>,
) -> Result<BatteryOptResult, String> {
    app.vpn()
        .request_ignore_battery_optimization()
        .map_err(|e| e.to_string())
}

#[command]
pub(crate) async fn open_oem_background_settings<R: Runtime>(
    app: AppHandle<R>,
) -> Result<OemSettingsResult, String> {
    app.vpn()
        .open_oem_background_settings()
        .map_err(|e| e.to_string())
}
