use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{error, info, warn};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::config;
use crate::config::generate_client_config;
use crate::models::{
    AppError, ConnectionInfo, ConnectionStatus, LogEntry, ServerConfig, SpeedStats,
};

const DEFAULT_SOCKS_PORT: u16 = 10808;
const MAX_LOG_ENTRIES: usize = 1000;

pub struct XrayManager {
    child: Arc<Mutex<Option<CommandChild>>>,
    state: Arc<Mutex<ConnectionInfo>>,
    config_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    stats: Arc<Mutex<SpeedStats>>,
    prev_uplink: Arc<Mutex<u64>>,
    prev_downlink: Arc<Mutex<u64>>,
    logs: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl Default for XrayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl XrayManager {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(ConnectionInfo::default())),
            config_path: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(SpeedStats::default())),
            prev_uplink: Arc::new(Mutex::new(0)),
            prev_downlink: Arc::new(Mutex::new(0)),
            logs: Arc::new(Mutex::new(VecDeque::new())),
        }
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

        // Generate xray config
        let config_json = generate_client_config(server, DEFAULT_SOCKS_PORT)?;

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
        let command = app
            .shell()
            .sidecar("xray")
            .map_err(|e| AppError::XrayProcess(format!("Failed to create sidecar command: {e}")))?
            .args(["run", "-c"])
            .arg(&config_file);

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
        let app_handle = app.clone();
        let server_name = server.name.clone();
        let server_address = server.address.clone();

        tauri::async_runtime::spawn(async move {
            let mut started = false;

            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        let line_str = String::from_utf8_lossy(&line);
                        let trimmed = line_str.trim();
                        info!("xray stdout: {}", trimmed);
                        push_log_entry(&logs_ref, "info", trimmed);
                    }
                    CommandEvent::Stderr(line) => {
                        let line_str = String::from_utf8_lossy(&line);
                        let trimmed = line_str.trim();
                        info!("xray stderr: {}", trimmed);

                        // Determine log level
                        let level = if trimmed.contains("[Warning]") {
                            "warning"
                        } else if trimmed.contains("[Error]") {
                            "error"
                        } else {
                            "info"
                        };
                        push_log_entry(&logs_ref, level, trimmed);

                        // xray logs to stderr; detect successful startup
                        if !started && trimmed.contains("started") {
                            started = true;
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

                        let mut s = state.lock().unwrap();
                        if s.status != ConnectionStatus::Disconnecting {
                            s.status = ConnectionStatus::Error;
                            s.error_message = Some(format!(
                                "xray exited unexpectedly (code: {:?})",
                                payload.code
                            ));
                        } else {
                            s.status = ConnectionStatus::Disconnected;
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

    pub fn stop(&self) -> Result<(), AppError> {
        self.update_status(ConnectionStatus::Disconnecting, None, None);

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

        // Update status
        self.update_status(ConnectionStatus::Disconnected, None, None);

        // Reset stats counters
        self.reset_stats();

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

        // xray may output to either stdout or stderr
        let combined = if stdout.contains(">>>") {
            &stdout
        } else {
            &stderr
        };

        let (uplink, downlink) = Self::parse_stats_output(combined);

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

    /// Parse xray statsquery output (protobuf text format)
    fn parse_stats_output(output: &str) -> (u64, u64) {
        let mut uplink: u64 = 0;
        let mut downlink: u64 = 0;

        let lines: Vec<&str> = output.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("outbound>>>proxy>>>traffic>>>uplink") {
                // Look for value on next line(s)
                for next_line in lines.iter().skip(i + 1).take(3) {
                    if let Some(val) = Self::extract_stat_value(next_line) {
                        uplink = val;
                        break;
                    }
                }
            } else if trimmed.contains("outbound>>>proxy>>>traffic>>>downlink") {
                for next_line in lines.iter().skip(i + 1).take(3) {
                    if let Some(val) = Self::extract_stat_value(next_line) {
                        downlink = val;
                        break;
                    }
                }
            }
        }

        (uplink, downlink)
    }

    fn extract_stat_value(line: &str) -> Option<u64> {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("value:") {
            rest.trim().parse().ok()
        } else {
            None
        }
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
