use std::path::Path;
use std::process::Command;

use log::{info, warn};

use crate::models::AppError;

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
            // No helper — build inline script
            let script = format!(
                "pkill -f hev-socks5-tunnel 2>/dev/null; \
                 ip route del default via {TUN_GW} dev {TUN_NAME} 2>/dev/null; \
                 ip link del {TUN_NAME} 2>/dev/null"
            );
            vec!["bash".to_string(), "-c".to_string(), script]
        };

        if gw_file.exists() {
            if let Ok(contents) = std::fs::read_to_string(&gw_file) {
                let lines: Vec<&str> = contents.lines().collect();
                if lines.len() >= 3 {
                    args.push(lines[0].to_string());
                    args.push(lines[1].to_string());
                    args.push(lines[2].to_string());
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

    Err(AppError::Config(format!(
        "rustvpn-helper not found. Run: sudo ./scripts/install-helper.sh"
    )))
}

/// Start TUN mode: write hev config, start hev-socks5-tunnel via pkexec helper, set up routes.
pub fn start_tun(
    hev_bin: &Path,
    socks_port: u16,
    server_ip: &str,
    bypass_subnets: &[String],
    config_dir: &Path,
) -> Result<(), AppError> {
    let helper = resolve_helper()?;

    // Detect current default gateway before we change routes
    let (gateway, dev) = detect_default_gateway()?;
    info!("Detected default gateway: {gateway} via {dev}");

    // Write hev-socks5-tunnel config
    let hev_config = config_dir.join("hev_config.yml");
    let pid_file = config_dir.join("hev.pid");
    write_hev_config(&hev_config, socks_port, &pid_file)?;

    // Save gateway info for stop_tun
    let gw_file = config_dir.join("tun_gateway.txt");
    std::fs::write(&gw_file, format!("{server_ip}\n{gateway}\n{dev}"))?;

    // Build args for helper (includes app PID for watchdog)
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

    // Read saved gateway info for bypass route cleanup
    if gw_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&gw_file) {
            let lines: Vec<&str> = contents.lines().collect();
            if lines.len() >= 3 {
                args.push(lines[0].to_string()); // server_ip
                args.push(lines[1].to_string()); // gateway
                args.push(lines[2].to_string()); // dev
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
