use std::path::Path;
use std::process::Command;

use log::{info, warn};

use crate::models::AppError;

/// Validate that a string looks like an IPv4 address (no shell metacharacters).
fn is_valid_ip(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok())
}

/// Validate that a string looks like a safe interface name.
fn is_valid_iface(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 16
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

const TUN_NAME: &str = "rvpn0";
const TUN_ADDR: &str = "198.18.0.1/15";
const TUN_GW: &str = "198.18.0.0";
const TUN_MTU: &str = "8500";
const HELPER_NAME: &str = "rustvpn-helper";

/// Check if a stale TUN device exists from a previous crash and clean it up.
pub fn cleanup_stale_tun(config_dir: &Path) {
    let tun_exists = Command::new("ip")
        .args(["link", "show", TUN_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !tun_exists {
        return;
    }

    warn!("Stale TUN device {TUN_NAME} found from previous session, cleaning up...");

    if let Err(e) = stop_tun(config_dir) {
        warn!("Normal TUN cleanup failed: {e}, attempting force cleanup...");
        // Force: kill hev by name, remove routes and TUN
        let helper = resolve_helper().ok();
        let pid_file = config_dir.join("hev.pid");
        let gw_file = config_dir.join("tun_gateway.txt");

        let mut args = if let Some(h) = helper {
            vec![h, "stop".to_string(), pid_file.to_string_lossy().to_string(),
                 TUN_NAME.to_string(), TUN_GW.to_string()]
        } else {
            // No helper — use individual commands (avoids shell injection via bash -c)
            let _ = Command::new("pkill").args(["-f", "hev-socks5-tunnel"]).output();
            let _ = Command::new("ip").args(["rule", "del", "lookup", "main", "priority", "100"]).output();
            let _ = Command::new("ip").args(["route", "del", "default", "via", TUN_GW, "dev", TUN_NAME]).output();
            let _ = Command::new("ip").args(["link", "del", TUN_NAME]).output();
            return;
        };

        if gw_file.exists() {
            if let Ok(contents) = std::fs::read_to_string(&gw_file) {
                let lines: Vec<&str> = contents.lines().collect();
                if lines.len() >= 3 {
                    let server_ip = lines[0];
                    let gateway = lines[1];
                    let dev = lines[2];
                    if is_valid_ip(server_ip) && is_valid_ip(gateway) && is_valid_iface(dev) {
                        args.push(server_ip.to_string());
                        args.push(gateway.to_string());
                        args.push(dev.to_string());
                        if lines.len() >= 4 && is_valid_ip(lines[3]) {
                            args.push(lines[3].to_string());
                        }
                    } else {
                        warn!("Stale gateway file contains invalid data, skipping route cleanup");
                    }
                }
            }
            let _ = std::fs::remove_file(&gw_file);
        }

        let _ = Command::new("pkexec").args(&args).output();
        let _ = std::fs::remove_file(config_dir.join("hev_config.yml"));
        let _ = std::fs::remove_file(&pid_file);
    }

    let still_exists = Command::new("ip")
        .args(["link", "show", TUN_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if still_exists {
        warn!("TUN device still exists after cleanup — may need manual intervention");
    } else {
        info!("Stale TUN device cleaned up successfully");
    }
}

/// Resolve the helper script path. Checks /usr/local/bin first (installed),
/// then falls back to the project's scripts/ directory (dev mode).
fn resolve_helper() -> Result<String, AppError> {
    let installed = format!("/usr/local/bin/{HELPER_NAME}");
    if std::path::Path::new(&installed).exists() {
        return Ok(installed);
    }

    // Dev mode: relative to CARGO_MANIFEST_DIR
    let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("scripts")
        .join(HELPER_NAME);
    if dev_path.exists() {
        return Ok(dev_path.to_string_lossy().to_string());
    }

    Err(AppError::Config(
        "rustvpn-helper not found. Run: sudo ./scripts/install-helper.sh".to_string(),
    ))
}

/// Start TUN mode: write hev config, start hev-socks5-tunnel via pkexec helper, set up routes.
pub fn start_tun(
    hev_bin: &Path,
    socks_port: u16,
    server_ip: &str,
    bypass_subnets: &[String],
    config_dir: &Path,
    gateway_info: Option<(String, String, String)>,
) -> Result<(), AppError> {
    let helper = resolve_helper()?;

    // Use pre-detected gateway info or detect now
    let (gateway, dev, local_ip) = match gateway_info {
        Some(info) => info,
        None => {
            let (gw, d) = detect_default_gateway()?;
            // Fallback: detect IP from the device
            let ip = crate::network::detect_default_gateway_and_ip()
                .map(|(_, _, ip)| ip)
                .unwrap_or_default();
            (gw, d, ip)
        }
    };

    if local_ip.is_empty() {
        return Err(AppError::Config(
            "Failed to detect local IP address for TUN routing. Cannot start TUN mode.".into(),
        ));
    }
    info!("Detected default gateway: {gateway} via {dev} (local IP: {local_ip})");

    // Write hev-socks5-tunnel config
    let hev_config = config_dir.join("hev_config.yml");
    let pid_file = config_dir.join("hev.pid");
    write_hev_config(&hev_config, socks_port, &pid_file)?;

    // Save gateway info for stop_tun (including local_ip for ip rule cleanup)
    let gw_file = config_dir.join("tun_gateway.txt");
    std::fs::write(&gw_file, format!("{server_ip}\n{gateway}\n{dev}\n{local_ip}"))?;

    // Build args for helper (includes app PID for watchdog and local IP for routing)
    let app_pid = std::process::id().to_string();
    let mut args = vec![
        helper.clone(),
        "start".to_string(),
        hev_bin.to_string_lossy().to_string(),
        hev_config.to_string_lossy().to_string(),
        pid_file.to_string_lossy().to_string(),
        TUN_NAME.to_string(),
        TUN_ADDR.to_string(),
        TUN_MTU.to_string(),
        TUN_GW.to_string(),
        server_ip.to_string(),
        gateway.clone(),
        dev.clone(),
        app_pid,
        local_ip,
    ];

    // Append bypass subnets as additional args
    for subnet in bypass_subnets {
        let s = subnet.trim();
        if !s.is_empty() {
            args.push(s.to_string());
        }
    }

    info!("Starting TUN via pkexec helper");

    let output = Command::new("pkexec")
        .args(&args)
        .output()
        .map_err(|e| AppError::XrayProcess(format!("Failed to start TUN (pkexec): {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Clean up gateway file on failure so stop_tun doesn't use stale data
        let _ = std::fs::remove_file(&gw_file);
        return Err(AppError::XrayProcess(format!(
            "TUN setup failed: {stderr}"
        )));
    }

    // Verify TUN interface is up
    if let Ok(check) = Command::new("ip")
        .args(["link", "show", TUN_NAME])
        .output()
    {
        if check.status.success() {
            info!("TUN device {TUN_NAME} is up");
        } else {
            warn!("TUN device {TUN_NAME} may not be up");
        }
    }

    Ok(())
}

/// Stop TUN mode: kill hev, remove routes and TUN device.
pub fn stop_tun(config_dir: &Path) -> Result<(), AppError> {
    let helper = resolve_helper()?;
    let pid_file = config_dir.join("hev.pid");
    let gw_file = config_dir.join("tun_gateway.txt");

    let mut args = vec![
        helper,
        "stop".to_string(),
        pid_file.to_string_lossy().to_string(),
        TUN_NAME.to_string(),
        TUN_GW.to_string(),
    ];

    // Read saved gateway info for bypass route and ip rule cleanup
    if gw_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&gw_file) {
            let lines: Vec<&str> = contents.lines().collect();
            if lines.len() >= 3 {
                let server_ip = lines[0];
                let gateway = lines[1];
                let dev = lines[2];
                // Validate all values before passing to privileged helper
                if is_valid_ip(server_ip) && is_valid_ip(gateway) && is_valid_iface(dev) {
                    args.push(server_ip.to_string());
                    args.push(gateway.to_string());
                    args.push(dev.to_string());
                    if lines.len() >= 4 && is_valid_ip(lines[3]) {
                        args.push(lines[3].to_string()); // local_ip
                    }
                } else {
                    warn!("Gateway file contains invalid data, skipping route cleanup");
                }
            }
        }
        let _ = std::fs::remove_file(&gw_file);
    }

    info!("Stopping TUN via pkexec helper");

    let output = Command::new("pkexec")
        .args(&args)
        .output()
        .map_err(|e| AppError::XrayProcess(format!("Failed to stop TUN (pkexec): {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("TUN cleanup had errors: {stderr}");
    } else {
        info!("TUN stopped and routes cleaned up");
    }

    // Clean up hev config
    let hev_config = config_dir.join("hev_config.yml");
    let _ = std::fs::remove_file(&hev_config);

    Ok(())
}

/// Detect the current default gateway and interface.
fn detect_default_gateway() -> Result<(String, String), AppError> {
    let output = Command::new("ip")
        .args(["-j", "route", "show", "default"])
        .output()
        .map_err(|e| AppError::Config(format!("Failed to run ip route: {e}")))?;

    if !output.status.success() {
        return Err(AppError::Config("ip route show default failed".into()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::Config(format!("Failed to parse ip route JSON: {e}")))?;

    let routes = json
        .as_array()
        .ok_or_else(|| AppError::Config("ip route returned non-array".into()))?;

    // Prefer physical (non-VPN) interface
    for route in routes {
        let dev = route.get("dev").and_then(|v| v.as_str());
        let gw = route.get("gateway").and_then(|v| v.as_str());

        if let (Some(gw), Some(dev)) = (gw, dev) {
            if !crate::network::is_vpn_interface(dev) {
                return Ok((gw.to_string(), dev.to_string()));
            }
        }
    }

    // Fallback: use any default route
    for route in routes {
        let dev = route.get("dev").and_then(|v| v.as_str());
        let gw = route.get("gateway").and_then(|v| v.as_str());

        if let (Some(gw), Some(dev)) = (gw, dev) {
            return Ok((gw.to_string(), dev.to_string()));
        }
    }

    Err(AppError::Config("No default gateway found".into()))
}

/// Write hev-socks5-tunnel YAML config file.
fn write_hev_config(
    path: &Path,
    socks_port: u16,
    pid_file: &Path,
) -> Result<(), AppError> {
    let config = format!(
        r#"tunnel:
  name: {TUN_NAME}
  mtu: {TUN_MTU}

socks5:
  port: {socks_port}
  address: 127.0.0.1
  udp: 'udp'

misc:
  task-stack-size: 81920
  connect-timeout: 5000
  read-write-timeout: 60000
  log-level: warn
  pid-file: {pid_file}
  limit-nofile: 65535
"#,
        pid_file = pid_file.display(),
    );

    std::fs::write(path, config)?;
    info!("Wrote hev config to {}", path.display());
    Ok(())
}
