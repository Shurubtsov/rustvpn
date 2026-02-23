use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
use desktop::VpnPlugin;
#[cfg(mobile)]
use mobile::VpnPlugin;

pub use commands::{VpnStats, VpnStatus};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Plugin invoke error: {0}")]
    PluginInvoke(String),
    #[error("Not supported on this platform")]
    NotSupported,
}

pub trait VpnPluginExt<R: Runtime> {
    fn vpn(&self) -> &VpnPlugin<R>;
}

impl<R: Runtime, T: Manager<R>> VpnPluginExt<R> for T {
    fn vpn(&self) -> &VpnPlugin<R> {
        self.state::<VpnPlugin<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("vpn")
        .invoke_handler(tauri::generate_handler![
            commands::start_vpn,
            commands::stop_vpn,
            commands::get_vpn_status,
            commands::query_stats,
        ])
        .setup(|app, _api| {
            #[cfg(mobile)]
            {
                let vpn = mobile::VpnPlugin::new(_api)?;
                app.manage(vpn);
            }
            #[cfg(desktop)]
            {
                app.manage(desktop::VpnPlugin::<R>::new());
            }
            Ok(())
        })
        .build()
}
