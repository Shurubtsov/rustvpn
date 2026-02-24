use std::process::Command;

use log::{error, info};

const SOCKS_HOST: &str = "127.0.0.1";
const HTTP_HOST: &str = "127.0.0.1";
const HTTP_PORT: u16 = 10809;

/// Enable system-wide proxy pointing to the local xray SOCKS5/HTTP proxy.
pub fn enable_system_proxy(socks_port: u16, bypass_domains: &[String], bypass_subnets: &[String]) {
    info!(
        "Enabling system proxy (SOCKS5: {}:{}, HTTP: {}:{})",
        SOCKS_HOST, socks_port, HTTP_HOST, HTTP_PORT
    );

    #[cfg(target_os = "linux")]
    enable_linux(socks_port, bypass_domains, bypass_subnets);

    #[cfg(target_os = "windows")]
    enable_windows(bypass_domains, bypass_subnets);

    #[cfg(target_os = "macos")]
    enable_macos(socks_port, bypass_domains, bypass_subnets);
}

/// Disable system-wide proxy.
pub fn disable_system_proxy() {
    info!("Disabling system proxy");

    #[cfg(target_os = "linux")]
    disable_linux();

    #[cfg(target_os = "windows")]
    disable_windows();

    #[cfg(target_os = "macos")]
    disable_macos();
}

// ---------------------------------------------------------------------------
// Linux (GNOME / gsettings)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn enable_linux(socks_port: u16, bypass_domains: &[String], bypass_subnets: &[String]) {
    if !has_gsettings() {
        info!("gsettings not found — skipping system proxy on Linux");
        return;
    }

    gsettings_set("org.gnome.system.proxy", "mode", "manual");

    // SOCKS proxy
    gsettings_set("org.gnome.system.proxy.socks", "host", SOCKS_HOST);
    gsettings_set(
        "org.gnome.system.proxy.socks",
        "port",
        &socks_port.to_string(),
    );

    // HTTP proxy
    gsettings_set("org.gnome.system.proxy.http", "host", HTTP_HOST);
    gsettings_set(
        "org.gnome.system.proxy.http",
        "port",
        &HTTP_PORT.to_string(),
    );

    // HTTPS proxy (same as HTTP)
    gsettings_set("org.gnome.system.proxy.https", "host", HTTP_HOST);
    gsettings_set(
        "org.gnome.system.proxy.https",
        "port",
        &HTTP_PORT.to_string(),
    );

    // Bypass list
    let mut hosts = vec![
        "'localhost'".to_string(),
        "'127.0.0.0/8'".to_string(),
        "'10.0.0.0/8'".to_string(),
        "'172.16.0.0/12'".to_string(),
        "'192.168.0.0/16'".to_string(),
        "'::1'".to_string(),
    ];
    for domain in bypass_domains {
        let d = domain.trim();
        if !d.is_empty() {
            hosts.push(format!("'{d}'"));
            hosts.push(format!("'*.{d}'"));
        }
    }
    for subnet in bypass_subnets {
        let s = subnet.trim();
        if !s.is_empty() {
            hosts.push(format!("'{s}'"));
        }
    }
    let ignore_hosts = format!("[{}]", hosts.join(", "));
    gsettings_set("org.gnome.system.proxy", "ignore-hosts", &ignore_hosts);

    info!("System proxy enabled via gsettings");
}

#[cfg(target_os = "linux")]
fn disable_linux() {
    if has_gsettings() {
        gsettings_set("org.gnome.system.proxy", "mode", "none");
        info!("System proxy disabled via gsettings");
    }
}

#[cfg(target_os = "linux")]
fn has_gsettings() -> bool {
    Command::new("which")
        .arg("gsettings")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn gsettings_set(schema: &str, key: &str, value: &str) {
    let result = Command::new("gsettings")
        .args(["set", schema, key, value])
        .output();

    match result {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("gsettings set {schema} {key} failed: {stderr}");
        }
        Err(e) => {
            error!("Failed to run gsettings: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Windows (registry + Internet Settings notification)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn enable_windows(bypass_domains: &[String], bypass_subnets: &[String]) {
    // Build bypass list in Windows format (semicolon-separated)
    let mut bypass = vec![
        "localhost".to_string(),
        "127.*".to_string(),
        "10.*".to_string(),
        "172.16.*".to_string(),
        "192.168.*".to_string(),
        "<local>".to_string(),
    ];
    for domain in bypass_domains {
        let d = domain.trim();
        if !d.is_empty() {
            bypass.push(d.to_string());
            bypass.push(format!("*.{d}"));
        }
    }
    for subnet in bypass_subnets {
        let s = subnet.trim();
        if !s.is_empty() {
            bypass.push(s.to_string());
        }
    }
    let bypass_str = bypass.join(";");
    let proxy_server = format!("{HTTP_HOST}:{HTTP_PORT}");

    // Set proxy via reg.exe (works without elevated privileges for current user)
    let base = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    reg_set(base, "ProxyEnable", "1");
    reg_set_str(base, "ProxyServer", &proxy_server);
    reg_set_str(base, "ProxyOverride", &bypass_str);

    // Notify the system that proxy settings changed so browsers pick it up immediately
    refresh_windows_proxy();

    info!("System proxy enabled via Windows registry ({})", proxy_server);
}

#[cfg(target_os = "windows")]
fn disable_windows() {
    let base = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    reg_set(base, "ProxyEnable", "0");
    // Remove ProxyServer and ProxyOverride
    reg_delete(base, "ProxyServer");
    reg_delete(base, "ProxyOverride");

    refresh_windows_proxy();

    info!("System proxy disabled via Windows registry");
}

#[cfg(target_os = "windows")]
fn reg_set(key: &str, name: &str, value: &str) {
    let result = Command::new("reg")
        .args(["add", key, "/v", name, "/t", "REG_DWORD", "/d", value, "/f"])
        .output();
    if let Err(e) = result {
        error!("reg add {name} failed: {e}");
    }
}

#[cfg(target_os = "windows")]
fn reg_set_str(key: &str, name: &str, value: &str) {
    let result = Command::new("reg")
        .args(["add", key, "/v", name, "/t", "REG_SZ", "/d", value, "/f"])
        .output();
    if let Err(e) = result {
        error!("reg add {name} failed: {e}");
    }
}

#[cfg(target_os = "windows")]
fn reg_delete(key: &str, name: &str) {
    // Ignore errors — value might not exist
    let _ = Command::new("reg")
        .args(["delete", key, "/v", name, "/f"])
        .output();
}

#[cfg(target_os = "windows")]
fn refresh_windows_proxy() {
    // Use PowerShell to call InternetSetOption to notify WinINet of the change.
    // This makes browsers (Chrome, Edge, etc.) pick up proxy changes immediately
    // without requiring a restart.
    let ps_script = r#"
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class WinINet {
    [DllImport("wininet.dll", SetLastError=true)]
    public static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int dwBufferLength);
    public const int INTERNET_OPTION_SETTINGS_CHANGED = 39;
    public const int INTERNET_OPTION_REFRESH = 37;
    public static void Refresh() {
        InternetSetOption(IntPtr.Zero, INTERNET_OPTION_SETTINGS_CHANGED, IntPtr.Zero, 0);
        InternetSetOption(IntPtr.Zero, INTERNET_OPTION_REFRESH, IntPtr.Zero, 0);
    }
}
"@
[WinINet]::Refresh()
"#;

    let result = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            info!("Windows proxy settings refreshed via InternetSetOption");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to refresh Windows proxy settings: {stderr}");
        }
        Err(e) => {
            error!("Failed to run PowerShell for proxy refresh: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// macOS (networksetup)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn enable_macos(socks_port: u16, bypass_domains: &[String], bypass_subnets: &[String]) {
    // Get active network service (usually "Wi-Fi" or "Ethernet")
    let service = match get_macos_network_service() {
        Some(s) => s,
        None => {
            error!("Could not detect active macOS network service");
            return;
        }
    };

    // Set HTTP proxy
    networksetup(&["-setwebproxy", &service, HTTP_HOST, &HTTP_PORT.to_string()]);
    networksetup(&["-setwebproxystate", &service, "on"]);

    // Set HTTPS proxy
    networksetup(&["-setsecurewebproxy", &service, HTTP_HOST, &HTTP_PORT.to_string()]);
    networksetup(&["-setsecurewebproxystate", &service, "on"]);

    // Set SOCKS proxy
    networksetup(&["-setsocksfirewallproxy", &service, SOCKS_HOST, &socks_port.to_string()]);
    networksetup(&["-setsocksfirewallproxystate", &service, "on"]);

    // Set bypass domains
    let mut bypass = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
        "::1".to_string(),
    ];
    for domain in bypass_domains {
        let d = domain.trim();
        if !d.is_empty() {
            bypass.push(d.to_string());
            bypass.push(format!("*.{d}"));
        }
    }
    for subnet in bypass_subnets {
        let s = subnet.trim();
        if !s.is_empty() {
            bypass.push(s.to_string());
        }
    }
    let mut args = vec!["-setproxybypassdomains".to_string(), service.clone()];
    args.extend(bypass);
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    networksetup(&args_refs);

    info!("System proxy enabled via networksetup (service: {})", service);
}

#[cfg(target_os = "macos")]
fn disable_macos() {
    let service = match get_macos_network_service() {
        Some(s) => s,
        None => {
            error!("Could not detect active macOS network service");
            return;
        }
    };

    networksetup(&["-setwebproxystate", &service, "off"]);
    networksetup(&["-setsecurewebproxystate", &service, "off"]);
    networksetup(&["-setsocksfirewallproxystate", &service, "off"]);

    info!("System proxy disabled via networksetup (service: {})", service);
}

#[cfg(target_os = "macos")]
fn get_macos_network_service() -> Option<String> {
    // Get the default route interface, then map it to a network service name
    let route_output = Command::new("route")
        .args(["-n", "get", "default"])
        .output()
        .ok()?;
    let route_str = String::from_utf8_lossy(&route_output.stdout);
    let iface = route_str
        .lines()
        .find(|l| l.contains("interface:"))?
        .split(':')
        .nth(1)?
        .trim()
        .to_string();

    // Map interface to service name
    let services_output = Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
        .ok()?;
    let services_str = String::from_utf8_lossy(&services_output.stdout);

    let mut current_service = String::new();
    for line in services_str.lines() {
        if let Some(name) = line.strip_prefix("Hardware Port: ") {
            current_service = name.to_string();
        } else if let Some(device) = line.strip_prefix("Device: ") {
            if device.trim() == iface {
                return Some(current_service);
            }
        }
    }

    // Fallback: try "Wi-Fi"
    Some("Wi-Fi".to_string())
}

#[cfg(target_os = "macos")]
fn networksetup(args: &[&str]) {
    let result = Command::new("networksetup").args(args).output();
    match result {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("networksetup {:?} failed: {stderr}", args);
        }
        Err(e) => {
            error!("Failed to run networksetup: {e}");
        }
    }
}
