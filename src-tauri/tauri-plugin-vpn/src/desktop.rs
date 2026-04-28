use crate::commands::{BatteryOptResult, BatteryOptStatus, OemSettingsResult, VpnStats, VpnStatus};

pub struct VpnPlugin<R: tauri::Runtime> {
    _phantom: std::marker::PhantomData<fn() -> R>,
}

impl<R: tauri::Runtime> VpnPlugin<R> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn start_vpn(
        &self,
        _config_json: String,
        _socks_port: u16,
        _server_address: String,
    ) -> Result<(), crate::Error> {
        Err(crate::Error::NotSupported)
    }

    pub fn stop_vpn(&self) -> Result<(), crate::Error> {
        Err(crate::Error::NotSupported)
    }

    pub fn get_status(&self) -> Result<VpnStatus, crate::Error> {
        Ok(VpnStatus {
            is_running: false,
            last_error: None,
            xray_running: false,
            hev_running: false,
            tun_active: false,
        })
    }

    pub fn query_stats(&self) -> Result<VpnStats, crate::Error> {
        Ok(VpnStats::default())
    }

    // Desktop platforms have no Doze / battery-optimization concept and no
    // OEM background-activity settings page, so these all return "yes, fine"
    // defaults. The frontend can call them unconditionally without platform
    // gating; the mobile prompts simply never trigger on desktop.
    pub fn is_battery_optimization_ignored(&self) -> Result<BatteryOptStatus, crate::Error> {
        Ok(BatteryOptStatus { ignored: true })
    }

    pub fn request_ignore_battery_optimization(&self) -> Result<BatteryOptResult, crate::Error> {
        Ok(BatteryOptResult { granted: true })
    }

    pub fn open_oem_background_settings(&self) -> Result<OemSettingsResult, crate::Error> {
        Ok(OemSettingsResult {
            opened: false,
            fallback: false,
        })
    }
}
