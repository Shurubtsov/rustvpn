use std::process::Command;

use log::{error, info};

const SOCKS_HOST: &str = "127.0.0.1";
const HTTP_HOST: &str = "127.0.0.1";
const HTTP_PORT: u16 = 10809;

/// Enable system-wide proxy pointing to the local xray SOCKS5/HTTP proxy.
pub fn enable_system_proxy(socks_port: u16) {
    info!(
        "Enabling system proxy (SOCKS5: {}:{}, HTTP: {}:{})",
        SOCKS_HOST, socks_port, HTTP_HOST, HTTP_PORT
    );

    // GNOME / GTK-based desktops (GNOME, Cinnamon, Budgie, MATE, XFCE with GNOME proxy)
    if has_gsettings() {
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

        // Bypass list for local addresses
        gsettings_set(
            "org.gnome.system.proxy",
            "ignore-hosts",
            "['localhost', '127.0.0.0/8', '10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16', '::1']",
        );

        info!("System proxy enabled via gsettings");
    }
}

/// Disable system-wide proxy.
pub fn disable_system_proxy() {
    info!("Disabling system proxy");

    if has_gsettings() {
        gsettings_set("org.gnome.system.proxy", "mode", "none");
        info!("System proxy disabled via gsettings");
    }
}

fn has_gsettings() -> bool {
    Command::new("which")
        .arg("gsettings")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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
