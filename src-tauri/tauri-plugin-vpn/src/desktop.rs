use crate::commands::{VpnStats, VpnStatus};

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
        })
    }

    pub fn query_stats(&self) -> Result<VpnStats, crate::Error> {
        Ok(VpnStats::default())
    }
}
