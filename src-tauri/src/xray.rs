use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{error, info, warn};
use tauri::{AppHandle, Emitter, Manager, Runtime};
#[cfg(desktop)]
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
#[cfg(desktop)]
use tauri_plugin_shell::ShellExt;

use crate::config;
use crate::config::generate_client_config;
use crate::models::{
    AppError, ConnectionInfo, ConnectionStatus, DetectedVpn, LogEntry, ServerConfig, SpeedStats,
};
#[cfg(desktop)]
use crate::network;
#[cfg(desktop)]
use crate::proxy;

const DEFAULT_SOCKS_PORT: u16 = 10808;
const MAX_LOG_ENTRIES: usize = 1000;

pub struct XrayManager {
    #[cfg(desktop)]
    child: Arc<Mutex<Option<CommandChild>>>,
    state: Arc<Mutex<ConnectionInfo>>,
    config_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    stats: Arc<Mutex<SpeedStats>>,
    prev_uplink: Arc<Mutex<u64>>,
    prev_downlink: Arc<Mutex<u64>>,
    logs: Arc<Mutex<VecDeque<LogEntry>>>,
    bypass_domains: Arc<Mutex<Vec<String>>>,
    bypass_subnets: Arc<Mutex<Vec<String>>>,
    detected_vpns: Arc<Mutex<Vec<DetectedVpn>>>,
}

impl Default for XrayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl XrayManager {
    pub fn new() -> Self {
        Self {
            #[cfg(desktop)]
            child: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(ConnectionInfo::default())),
            config_path: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(SpeedStats::default())),
            prev_uplink: Arc::new(Mutex::new(0)),
            prev_downlink: Arc::new(Mutex::new(0)),
            logs: Arc::new(Mutex::new(VecDeque::new())),
            bypass_domains: Arc::new(Mutex::new(Vec::new())),
            bypass_subnets: Arc::new(Mutex::new(Vec::new())),
            detected_vpns: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return the last detected VPN interfaces.
    pub fn get_detected_vpns(&self) -> Vec<DetectedVpn> {
        self.detected_vpns.lock().unwrap().clone()
    }

    pub fn status(&self) -> ConnectionInfo {
        self.state.lock().unwrap().clone()
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.logs.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear_logs(&self) {
        self.logs.lock().unwrap().clear();
    }

    pub fn start<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        server: &ServerConfig,
        bypass_domains: &[String],
    ) -> Result<(), AppError> {
        // Don't start if already running
        {
            let current = self.state.lock().unwrap();
            if current.status == ConnectionStatus::Connected
                || current.status == ConnectionStatus::Connecting
            {
                return Err(AppError::XrayProcess(
                    "Already connected or connecting".to_string(),
                ));
            }
        }

        // Reset stats counters for new connection
        self.reset_stats();

        // Update status to connecting
        self.update_status(ConnectionStatus::Connecting, Some(server), None);

        #[cfg(desktop)]
        {
            self.start_desktop(app, server, bypass_domains)?;
        }

        #[cfg(mobile)]
        {
            self.start_mobile(app, server, bypass_domains)?;
        }

        Ok(())
    }

    #[cfg(desktop)]
    fn start_desktop<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        server: &ServerConfig,
        bypass_domains: &[String],
    ) -> Result<(), AppError> {
        // Kill any stale xray process from a previous run
        {
            let mut guard = self.child.lock().unwrap();
            if let Some(child) = guard.take() {
                let _ = child.kill();
                info!("Killed stale xray process");
            }
        }

        // Detect corporate VPN interfaces and collect bypass subnets
        let vpns = network::detect_vpn_routes();
        let bypass_subnet_list = network::collect_bypass_subnets(&vpns);

        // Store detected VPNs and bypass subnets
        {
            let mut dv = self.detected_vpns.lock().unwrap();
            *dv = vpns;
        }
        {
            let mut bs = self.bypass_subnets.lock().unwrap();
            *bs = bypass_subnet_list.clone();
        }

        // Store bypass domains for proxy setup
        {
            let mut bd = self.bypass_domains.lock().unwrap();
            *bd = bypass_domains.to_vec();
        }

        // Generate xray config
        let config_json =
            generate_client_config(server, DEFAULT_SOCKS_PORT, bypass_domains, &bypass_subnet_list)?;

        // Write config to temp file
        let config_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Config(format!("Failed to get app data dir: {e}")))?;
        std::fs::create_dir_all(&config_dir)?;
        let config_file = config_dir.join("xray_config.json");
        std::fs::write(&config_file, &config_json)?;

        info!("Wrote xray config to {}", config_file.display());

        // Store config path for cleanup
        {
            let mut path = self.config_path.lock().unwrap();
            *path = Some(config_file.clone());
        }

        // Create sidecar command
        let config_path_str = config_file.to_string_lossy().to_string();
        let command = app
            .shell()
            .sidecar("xray")
            .map_err(|e| AppError::XrayProcess(format!("Failed to create sidecar command: {e}")))?
            .args(["run", "-c", &config_path_str]);

        // Spawn the process
        let (mut rx, child_process) = command
            .spawn()
            .map_err(|e| AppError::XrayProcess(format!("Failed to spawn xray: {e}")))?;

        info!("Spawned xray process with PID {}", child_process.pid());

        // Store child handle
        {
            let mut guard = self.child.lock().unwrap();
            *guard = Some(child_process);
        }

        // Monitor output in background
        let state = self.state.clone();
        let child_ref = self.child.clone();
        let logs_ref = self.logs.clone();
        let bypass_ref = self.bypass_domains.clone();
        let bypass_subnets_ref = self.bypass_subnets.clone();
        let app_handle = app.clone();
        let server_name = server.name.clone();
        let server_address = server.address.clone();

        // Shared flag for timeout coordination
        let started_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_flag_clone = started_flag.clone();

        // Connection timeout: kill xray if not started within 15 seconds
        let timeout_state = self.state.clone();
        let timeout_child = self.child.clone();
        let timeout_logs = self.logs.clone();
        let timeout_app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(15));
            if !started_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let mut s = timeout_state.lock().unwrap();
                if s.status == ConnectionStatus::Connecting {
                    warn!("Connection timeout after 15 seconds");
                    push_log_entry(&timeout_logs, "error", "Connection timeout after 15 seconds");
                    s.status = ConnectionStatus::Error;
                    s.error_message =
                        Some("Connection timeout — server unreachable or config invalid".to_string());
                    s.connected_since = None;
                    drop(s);
                    // Kill the xray process
                    let child = { timeout_child.lock().unwrap().take() };
                    if let Some(child) = child {
                        let _ = child.kill();
                    }
                    let _ = timeout_app.emit("connection-status-changed", "disconnected");
                }
            }
        });

        tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        let line_str = String::from_utf8_lossy(&line);
                        let trimmed = line_str.trim();
                        info!("xray stdout: {}", trimmed);
                        push_log_entry(&logs_ref, "info", trimmed);

                        if !started_flag.load(std::sync::atomic::Ordering::Relaxed)
                            && trimmed.contains("started")
                        {
                            started_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            let mut s = state.lock().unwrap();
                            if s.status == ConnectionStatus::Connecting {
                                s.status = ConnectionStatus::Connected;
                                s.connected_since = Some(now);
                                s.server_name = Some(server_name.clone());
                                s.server_address = Some(server_address.clone());
                                s.error_message = None;
                                info!("xray connected successfully (detected from stdout)");
                                proxy::enable_system_proxy(DEFAULT_SOCKS_PORT, &bypass_ref.lock().unwrap(), &bypass_subnets_ref.lock().unwrap());
                            }
                            drop(s);
                            let _ = app_handle.emit("connection-status-changed", "connected");
                        }
                    }
                    CommandEvent::Stderr(line) => {
                        let line_str = String::from_utf8_lossy(&line);
                        let trimmed = line_str.trim();
                        info!("xray stderr: {}", trimmed);

                        let level = if trimmed.contains("[Warning]") {
                            "warning"
                        } else if trimmed.contains("[Error]") {
                            "error"
                        } else {
                            "info"
                        };
                        push_log_entry(&logs_ref, level, trimmed);

                        if !started_flag.load(std::sync::atomic::Ordering::Relaxed)
                            && trimmed.contains("started")
                        {
                            started_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            let mut s = state.lock().unwrap();
                            if s.status == ConnectionStatus::Connecting {
                                s.status = ConnectionStatus::Connected;
                                s.connected_since = Some(now);
                                s.server_name = Some(server_name.clone());
                                s.server_address = Some(server_address.clone());
                                s.error_message = None;
                                info!("xray connected successfully");
                                proxy::enable_system_proxy(DEFAULT_SOCKS_PORT, &bypass_ref.lock().unwrap(), &bypass_subnets_ref.lock().unwrap());
                            }
                            drop(s);
                            let _ = app_handle.emit("connection-status-changed", "connected");
                        }
                    }
                    CommandEvent::Error(err) => {
                        error!("xray error event: {}", err);
                        push_log_entry(&logs_ref, "error", &err);
                    }
                    CommandEvent::Terminated(payload) => {
                        warn!(
                            "xray terminated with code: {:?}, signal: {:?}",
                            payload.code, payload.signal
                        );
                        let msg = format!(
                            "xray terminated (code: {:?}, signal: {:?})",
                            payload.code, payload.signal
                        );
                        push_log_entry(&logs_ref, "warning", &msg);

                        proxy::disable_system_proxy();

                        let mut s = state.lock().unwrap();
                        if s.status == ConnectionStatus::Disconnecting {
                            s.status = ConnectionStatus::Disconnected;
                        } else if s.status != ConnectionStatus::Disconnected
                            && s.status != ConnectionStatus::Error
                        {
                            s.status = ConnectionStatus::Error;
                            s.error_message = Some(format!(
                                "xray exited unexpectedly (code: {:?})",
                                payload.code
                            ));
                        }
                        s.connected_since = None;

                        let mut c = child_ref.lock().unwrap();
                        *c = None;
                        drop(s);
                        drop(c);
                        let _ = app_handle.emit("connection-status-changed", "disconnected");
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    #[cfg(mobile)]
    fn start_mobile<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        server: &ServerConfig,
        bypass_domains: &[String],
    ) -> Result<(), AppError> {
        use tauri_plugin_vpn::VpnPluginExt;

        // Generate xray config (no bypass subnets on mobile)
        let mut config_json =
            generate_client_config(server, DEFAULT_SOCKS_PORT, bypass_domains, &[])?;

        // Apply Android-specific modifications
        config_json = config::modify_config_for_android(&config_json)?;

        // Start VPN via plugin
        app.vpn()
            .start_vpn(config_json, DEFAULT_SOCKS_PORT, server.address.clone())
            .map_err(|e| AppError::XrayProcess(format!("VPN plugin error: {e}")))?;

        // Update status to connected (the actual connection happens asynchronously in the service,
        // but we optimistically set it here; the frontend will poll for real status)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut state = self.state.lock().unwrap();
        state.status = ConnectionStatus::Connected;
        state.connected_since = Some(now);
        state.server_name = Some(server.name.clone());
        state.server_address = Some(server.address.clone());
        state.error_message = None;

        Ok(())
    }

    pub fn stop(&self) -> Result<(), AppError> {
        self.update_status(ConnectionStatus::Disconnecting, None, None);

        #[cfg(desktop)]
        {
            self.stop_desktop()?;
        }

        #[cfg(mobile)]
        {
            self.stop_mobile()?;
        }

        // Update status
        self.update_status(ConnectionStatus::Disconnected, None, None);

        // Reset stats counters
        self.reset_stats();

        Ok(())
    }

    #[cfg(desktop)]
    fn stop_desktop(&self) -> Result<(), AppError> {
        // Disable system proxy immediately
        proxy::disable_system_proxy();

        // Kill the child process
        let child = {
            let mut guard = self.child.lock().unwrap();
            guard.take()
        };

        if let Some(child) = child {
            child
                .kill()
                .map_err(|e| AppError::XrayProcess(format!("Failed to kill xray: {e}")))?;
            info!("Killed xray process");
        }

        // Clean up config file
        let config_path = {
            let mut guard = self.config_path.lock().unwrap();
            guard.take()
        };
        if let Some(path) = config_path {
            if path.exists() {
                let _ = std::fs::remove_file(&path);
                info!("Removed config file: {}", path.display());
            }
        }

        Ok(())
    }

    #[cfg(mobile)]
    fn stop_mobile(&self) -> Result<(), AppError> {
        // On mobile, we can't call the plugin here directly without an AppHandle.
        // The stop is triggered via the command layer which calls the plugin.
        // This method just handles state cleanup.
        Ok(())
    }

    pub fn test_connection(&self) -> Result<bool, AppError> {
        // Try to connect through the SOCKS5 proxy to verify it's working
        let addr = format!("127.0.0.1:{DEFAULT_SOCKS_PORT}");
        match std::net::TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(3)) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn socks_port(&self) -> u16 {
        DEFAULT_SOCKS_PORT
    }

    /// Query stats from xray's gRPC API via the sidecar binary
    pub async fn query_stats<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<SpeedStats, AppError> {
        // Only query if connected
        {
            let state = self.state.lock().unwrap();
            if state.status != ConnectionStatus::Connected {
                return Ok(SpeedStats::default());
            }
        }

        #[cfg(desktop)]
        let (uplink, downlink) = {
            // Run xray api statsquery via sidecar
            let output = app
                .shell()
                .sidecar("xray")
                .map_err(|e| AppError::XrayProcess(format!("Failed to create sidecar command: {e}")))?
                .args(["api", "statsquery", "-s", config::STATS_API_ADDR])
                .output()
                .await
                .map_err(|e| AppError::XrayProcess(format!("Failed to query stats: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let combined = if stdout.contains(">>>") {
                &stdout
            } else {
                &stderr
            };

            Self::parse_stats_output(combined)
        };

        #[cfg(mobile)]
        let (uplink, downlink) = {
            use tauri_plugin_vpn::VpnPluginExt;
            let stats = app.vpn().query_stats().map_err(|e| {
                AppError::XrayProcess(format!("Failed to query mobile stats: {e}"))
            })?;
            (stats.upload, stats.download)
        };

        // Compute speed from delta
        let mut prev_up = self.prev_uplink.lock().unwrap();
        let mut prev_down = self.prev_downlink.lock().unwrap();

        let upload_speed = uplink.saturating_sub(*prev_up);
        let download_speed = downlink.saturating_sub(*prev_down);

        *prev_up = uplink;
        *prev_down = downlink;

        let new_stats = SpeedStats {
            upload_speed,
            download_speed,
            total_upload: uplink,
            total_download: downlink,
        };

        // Update stored stats
        {
            let mut stats = self.stats.lock().unwrap();
            *stats = new_stats.clone();
        }

        Ok(new_stats)
    }

    /// Get cached stats without querying (for non-async contexts)
    pub fn cached_stats(&self) -> SpeedStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset stats counters (called on connect/disconnect)
    fn reset_stats(&self) {
        let mut stats = self.stats.lock().unwrap();
        *stats = SpeedStats::default();
        let mut prev_up = self.prev_uplink.lock().unwrap();
        *prev_up = 0;
        let mut prev_down = self.prev_downlink.lock().unwrap();
        *prev_down = 0;
    }

    /// Parse xray statsquery JSON output
    fn parse_stats_output(output: &str) -> (u64, u64) {
        let mut uplink: u64 = 0;
        let mut downlink: u64 = 0;

        // Output is JSON: {"stat": [{"name": "...", "value": N}, ...]}
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(stats) = json.get("stat").and_then(|s| s.as_array()) {
                for entry in stats {
                    let name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let value = entry
                        .get("value")
                        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                        .unwrap_or(0);

                    if name == "outbound>>>proxy>>>traffic>>>uplink" {
                        uplink = value;
                    } else if name == "outbound>>>proxy>>>traffic>>>downlink" {
                        downlink = value;
                    }
                }
            }
        }

        (uplink, downlink)
    }

    fn update_status(
        &self,
        status: ConnectionStatus,
        server: Option<&ServerConfig>,
        error: Option<String>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.status = status;
        if let Some(srv) = server {
            state.server_name = Some(srv.name.clone());
            state.server_address = Some(srv.address.clone());
        }
        if status == ConnectionStatus::Disconnected {
            state.server_name = None;
            state.server_address = None;
            state.connected_since = None;
            state.error_message = None;
        }
        if let Some(err) = error {
            state.error_message = Some(err);
        }
    }
}

fn push_log_entry(logs: &Arc<Mutex<VecDeque<LogEntry>>>, level: &str, message: &str) {
    let entry = LogEntry {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        level: level.to_string(),
        message: message.to_string(),
    };
    let mut buffer = logs.lock().unwrap();
    if buffer.len() >= MAX_LOG_ENTRIES {
        buffer.pop_front();
    }
    buffer.push_back(entry);
}
