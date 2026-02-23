use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::commands::{VpnStats, VpnStatus};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.rustvpn.vpn";

pub struct VpnPlugin<R: Runtime> {
    handle: PluginHandle<R>,
}

impl<R: Runtime> VpnPlugin<R> {
    pub fn new(api: PluginApi<R, ()>) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "android")]
        let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "VpnPlugin")?;
        #[cfg(target_os = "ios")]
        let handle = api.register_ios_plugin(init_plugin_vpn)?;
        Ok(Self { handle })
    }

    pub fn start_vpn(&self, config_json: String, socks_port: u16) -> Result<(), crate::Error> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "startVpn",
                serde_json::json!({
                    "configJson": config_json,
                    "socksPort": socks_port,
                }),
            )
            .map_err(|e| crate::Error::PluginInvoke(e.to_string()))?;
        Ok(())
    }

    pub fn stop_vpn(&self) -> Result<(), crate::Error> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("stopVpn", serde_json::json!({}))
            .map_err(|e| crate::Error::PluginInvoke(e.to_string()))?;
        Ok(())
    }

    pub fn get_status(&self) -> Result<VpnStatus, crate::Error> {
        self.handle
            .run_mobile_plugin::<VpnStatus>("getVpnStatus", serde_json::json!({}))
            .map_err(|e| crate::Error::PluginInvoke(e.to_string()))
    }

    pub fn query_stats(&self) -> Result<VpnStats, crate::Error> {
        self.handle
            .run_mobile_plugin::<VpnStats>("queryStats", serde_json::json!({}))
            .map_err(|e| crate::Error::PluginInvoke(e.to_string()))
    }
}
