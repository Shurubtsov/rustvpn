use std::process::Command;

use log::{info, warn};
use serde::Deserialize;

pub use crate::models::DetectedVpn;

/// A single route entry from `ip -j route show`.
#[derive(Debug, Clone, Deserialize)]
struct IpRoute {
    dst: Option<String>,
    dev: Option<String>,
    gateway: Option<String>,
    protocol: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    scope: Option<String>,
}

/// Detect active VPN interfaces and their routed subnets by parsing `ip -j route show`.
pub fn detect_vpn_routes() -> Vec<DetectedVpn> {
    let output = match Command::new("ip").args(["-j", "route", "show"]).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            warn!("ip route show failed: {stderr}");
            return Vec::new();
        }
        Err(e) => {
            warn!("Failed to run ip command: {e}");
            return Vec::new();
        }
    };

    let vpns = parse_routes_json(&output);

    if vpns.is_empty() {
        info!("No corporate VPN interfaces detected");
    } else {
        info!(
            "Detected {} corporate VPN interface(s): {}",
            vpns.len(),
            vpns.iter()
                .map(|v| format!("{} ({})", v.interface, v.vpn_type))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    vpns
}

/// Flatten all detected VPN subnets and server IPs into a single bypass list.
pub fn collect_bypass_subnets(vpns: &[DetectedVpn]) -> Vec<String> {
    let mut result = Vec::new();
    for vpn in vpns {
        result.extend(vpn.subnets.clone());
        if let Some(ref ip) = vpn.server_ip {
            // Add as host route
            if !ip.contains('/') {
                result.push(format!("{ip}/32"));
            } else {
                result.push(ip.clone());
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

/// Check whether an interface name looks like a VPN interface.
pub fn is_vpn_interface(name: &str) -> bool {
    let prefixes = ["tun", "tap", "wg", "ppp", "nordlynx", "tailscale"];
    prefixes.iter().any(|p| name.starts_with(p))
}

/// Pure function: parse `ip -j route show` JSON output into detected VPNs.
/// Separated from system calls for testability.
pub fn parse_routes_json(json: &str) -> Vec<DetectedVpn> {
    let routes: Vec<IpRoute> = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse ip route JSON: {e}");
            return Vec::new();
        }
    };

    // Collect subnets per VPN interface
    let mut vpn_subnets: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    // Collect potential VPN server endpoints (host routes through physical interfaces)
    let mut server_endpoints: Vec<String> = Vec::new();

    for route in &routes {
        let dev = match route.dev.as_deref() {
            Some(d) => d,
            None => continue,
        };
        let dst = match route.dst.as_deref() {
            Some(d) => d,
            None => continue,
        };

        if is_vpn_interface(dev) {
            // Skip default/catch-all routes
            if is_default_route(dst) {
                continue;
            }
            vpn_subnets
                .entry(dev.to_string())
                .or_default()
                .push(dst.to_string());
        } else {
            // Look for VPN server endpoint: host route (/32 or bare IP) with static protocol
            if is_host_route(dst)
                && route.protocol.as_deref() == Some("static")
                && route.gateway.is_some()
            {
                let ip = dst.trim_end_matches("/32").to_string();
                server_endpoints.push(ip);
            }
        }
    }

    // Build result
    let mut vpns: Vec<DetectedVpn> = vpn_subnets
        .into_iter()
        .map(|(iface, subnets)| {
            let vpn_type = classify_vpn_type(&iface);
            DetectedVpn {
                interface: iface,
                vpn_type,
                subnets,
                server_ip: None,
            }
        })
        .collect();

    // Sort for deterministic output
    vpns.sort_by(|a, b| a.interface.cmp(&b.interface));

    // Assign server endpoints — if there are VPN interfaces detected and static host routes,
    // the host routes are likely VPN server protection entries
    if !vpns.is_empty() && !server_endpoints.is_empty() {
        // Assign server IP to the first VPN (heuristic; most setups have one corporate VPN)
        vpns[0].server_ip = Some(server_endpoints.remove(0));
    }

    vpns
}

/// Check if a route destination is a default/catch-all route.
fn is_default_route(dst: &str) -> bool {
    matches!(
        dst,
        "default" | "0.0.0.0/0" | "0.0.0.0/1" | "128.0.0.0/1"
    )
}

/// Check if a route destination is a host route (/32 or bare IP without subnet).
fn is_host_route(dst: &str) -> bool {
    if dst.ends_with("/32") {
        return true;
    }
    // Bare IP address (no slash) — also a host route
    if !dst.contains('/') && dst.contains('.') {
        // Validate it looks like an IP
        dst.split('.').count() == 4 && dst.split('.').all(|p| p.parse::<u8>().is_ok())
    } else {
        false
    }
}

/// Classify VPN type from interface name.
fn classify_vpn_type(iface: &str) -> String {
    if iface.starts_with("tun") {
        "OpenVPN".to_string()
    } else if iface.starts_with("tap") {
        "OpenVPN (TAP)".to_string()
    } else if iface.starts_with("wg") {
        "WireGuard".to_string()
    } else if iface.starts_with("ppp") {
        "PPP/L2TP".to_string()
    } else if iface.starts_with("nordlynx") {
        "NordVPN".to_string()
    } else if iface.starts_with("tailscale") {
        "Tailscale".to_string()
    } else {
        "Unknown VPN".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vpn_routes_with_tun() {
        let json = r#"[
            {"dst": "default", "gateway": "192.168.1.1", "dev": "wlp2s0", "protocol": "dhcp"},
            {"dst": "10.8.0.0/24", "dev": "tun0", "protocol": "kernel", "scope": "link"},
            {"dst": "172.20.0.0/16", "dev": "tun0", "gateway": "10.8.0.1", "protocol": "static"},
            {"dst": "192.168.1.0/24", "dev": "wlp2s0", "protocol": "kernel", "scope": "link"}
        ]"#;

        let vpns = parse_routes_json(json);
        assert_eq!(vpns.len(), 1);
        assert_eq!(vpns[0].interface, "tun0");
        assert_eq!(vpns[0].vpn_type, "OpenVPN");
        assert_eq!(vpns[0].subnets.len(), 2);
        assert!(vpns[0].subnets.contains(&"10.8.0.0/24".to_string()));
        assert!(vpns[0].subnets.contains(&"172.20.0.0/16".to_string()));
    }

    #[test]
    fn test_parse_vpn_routes_wireguard() {
        let json = r#"[
            {"dst": "default", "gateway": "192.168.1.1", "dev": "eth0", "protocol": "dhcp"},
            {"dst": "10.0.0.0/8", "dev": "wg0", "protocol": "kernel", "scope": "link"},
            {"dst": "192.168.1.0/24", "dev": "eth0", "protocol": "kernel", "scope": "link"}
        ]"#;

        let vpns = parse_routes_json(json);
        assert_eq!(vpns.len(), 1);
        assert_eq!(vpns[0].interface, "wg0");
        assert_eq!(vpns[0].vpn_type, "WireGuard");
        assert_eq!(vpns[0].subnets, vec!["10.0.0.0/8"]);
    }

    #[test]
    fn test_no_vpn_interfaces() {
        let json = r#"[
            {"dst": "default", "gateway": "192.168.1.1", "dev": "wlp2s0", "protocol": "dhcp"},
            {"dst": "192.168.1.0/24", "dev": "wlp2s0", "protocol": "kernel", "scope": "link"},
            {"dst": "169.254.0.0/16", "dev": "wlp2s0", "protocol": "kernel", "scope": "link"}
        ]"#;

        let vpns = parse_routes_json(json);
        assert!(vpns.is_empty());
    }

    #[test]
    fn test_is_vpn_interface() {
        assert!(is_vpn_interface("tun0"));
        assert!(is_vpn_interface("tun1"));
        assert!(is_vpn_interface("tap0"));
        assert!(is_vpn_interface("wg0"));
        assert!(is_vpn_interface("wg1"));
        assert!(is_vpn_interface("ppp0"));
        assert!(is_vpn_interface("nordlynx"));
        assert!(is_vpn_interface("tailscale0"));

        assert!(!is_vpn_interface("eth0"));
        assert!(!is_vpn_interface("wlp2s0"));
        assert!(!is_vpn_interface("enp3s0"));
        assert!(!is_vpn_interface("lo"));
        assert!(!is_vpn_interface("docker0"));
        assert!(!is_vpn_interface("br-abc123"));
    }

    #[test]
    fn test_skips_default_routes() {
        let json = r#"[
            {"dst": "default", "dev": "tun0", "gateway": "10.8.0.1"},
            {"dst": "0.0.0.0/0", "dev": "tun0", "gateway": "10.8.0.1"},
            {"dst": "0.0.0.0/1", "dev": "tun0", "gateway": "10.8.0.1"},
            {"dst": "128.0.0.0/1", "dev": "tun0", "gateway": "10.8.0.1"},
            {"dst": "10.8.0.0/24", "dev": "tun0", "protocol": "kernel", "scope": "link"}
        ]"#;

        let vpns = parse_routes_json(json);
        assert_eq!(vpns.len(), 1);
        // Only the non-default route should be included
        assert_eq!(vpns[0].subnets, vec!["10.8.0.0/24"]);
    }

    #[test]
    fn test_detects_server_endpoint() {
        let json = r#"[
            {"dst": "default", "gateway": "192.168.1.1", "dev": "wlp2s0", "protocol": "dhcp"},
            {"dst": "10.8.0.0/24", "dev": "tun0", "protocol": "kernel", "scope": "link"},
            {"dst": "185.100.50.25/32", "gateway": "192.168.1.1", "dev": "wlp2s0", "protocol": "static"},
            {"dst": "192.168.1.0/24", "dev": "wlp2s0", "protocol": "kernel", "scope": "link"}
        ]"#;

        let vpns = parse_routes_json(json);
        assert_eq!(vpns.len(), 1);
        assert_eq!(vpns[0].server_ip, Some("185.100.50.25".to_string()));
    }

    #[test]
    fn test_collect_bypass_subnets() {
        let vpns = vec![
            DetectedVpn {
                interface: "tun0".to_string(),
                vpn_type: "OpenVPN".to_string(),
                subnets: vec!["10.8.0.0/24".to_string(), "172.20.0.0/16".to_string()],
                server_ip: Some("185.100.50.25".to_string()),
            },
            DetectedVpn {
                interface: "wg0".to_string(),
                vpn_type: "WireGuard".to_string(),
                subnets: vec!["10.0.0.0/8".to_string()],
                server_ip: None,
            },
        ];

        let subnets = collect_bypass_subnets(&vpns);
        assert!(subnets.contains(&"10.8.0.0/24".to_string()));
        assert!(subnets.contains(&"172.20.0.0/16".to_string()));
        assert!(subnets.contains(&"10.0.0.0/8".to_string()));
        assert!(subnets.contains(&"185.100.50.25/32".to_string()));
        assert_eq!(subnets.len(), 4);
    }

    #[test]
    fn test_collect_bypass_subnets_deduplicates() {
        let vpns = vec![
            DetectedVpn {
                interface: "tun0".to_string(),
                vpn_type: "OpenVPN".to_string(),
                subnets: vec!["10.0.0.0/8".to_string()],
                server_ip: None,
            },
            DetectedVpn {
                interface: "wg0".to_string(),
                vpn_type: "WireGuard".to_string(),
                subnets: vec!["10.0.0.0/8".to_string()],
                server_ip: None,
            },
        ];

        let subnets = collect_bypass_subnets(&vpns);
        assert_eq!(subnets.len(), 1);
        assert_eq!(subnets[0], "10.0.0.0/8");
    }

    #[test]
    fn test_empty_json_array() {
        let vpns = parse_routes_json("[]");
        assert!(vpns.is_empty());
    }

    #[test]
    fn test_invalid_json() {
        let vpns = parse_routes_json("not json at all");
        assert!(vpns.is_empty());
    }

    #[test]
    fn test_host_route_bare_ip() {
        let json = r#"[
            {"dst": "10.8.0.0/24", "dev": "tun0", "protocol": "kernel", "scope": "link"},
            {"dst": "203.0.113.5", "gateway": "192.168.1.1", "dev": "eth0", "protocol": "static"}
        ]"#;

        let vpns = parse_routes_json(json);
        assert_eq!(vpns.len(), 1);
        assert_eq!(vpns[0].server_ip, Some("203.0.113.5".to_string()));
    }
}
