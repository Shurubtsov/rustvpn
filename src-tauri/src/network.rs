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
/// When any corporate VPN is active, always prepend all RFC-1918 ranges so that
/// corporate DNS servers and internal servers bypass the RustVPN TUN and use the
/// corporate VPN's correct source IP via the main routing table.
pub fn collect_bypass_subnets(vpns: &[DetectedVpn]) -> Vec<String> {
    if vpns.is_empty() {
        return Vec::new();
    }

    let mut result = vec![
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
    ];

    for vpn in vpns {
        result.extend(vpn.subnets.clone());
        if let Some(ref ip) = vpn.server_ip {
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

/// Detect default gateway (preferring physical interfaces) and the local IP of that interface.
/// Returns (gateway_ip, device_name, local_ip).
pub fn detect_default_gateway_and_ip() -> Option<(String, String, String)> {
    // Get default route
    let route_out = Command::new("ip")
        .args(["-j", "route", "show", "default"])
        .output()
        .ok()?;
    if !route_out.status.success() {
        return None;
    }
    let routes: Vec<IpRoute> = serde_json::from_slice(&route_out.stdout).ok()?;

    // Find physical (non-VPN, non-virtual) default route
    let (gw, dev) = routes
        .iter()
        .filter_map(|r| {
            let gw = r.gateway.as_deref()?;
            let dev = r.dev.as_deref()?;
            if !is_vpn_interface(dev) && !is_virtual_interface(dev) {
                Some((gw.to_string(), dev.to_string()))
            } else {
                None
            }
        })
        .next()
        .or_else(|| {
            routes
                .iter()
                .filter_map(|r| {
                    let gw = r.gateway.as_deref()?;
                    let dev = r.dev.as_deref()?;
                    Some((gw.to_string(), dev.to_string()))
                })
                .next()
        })?;

    // Get IP address of that interface
    let addr_out = Command::new("ip")
        .args(["-j", "-4", "addr", "show", "dev", &dev])
        .output()
        .ok()?;
    if !addr_out.status.success() {
        return None;
    }
    let addr_json: serde_json::Value = serde_json::from_slice(&addr_out.stdout).ok()?;
    let local_ip = addr_json
        .as_array()?
        .first()?
        .get("addr_info")?
        .as_array()?
        .iter()
        .filter_map(|info| info.get("local")?.as_str())
        .find(|ip| !is_link_local(ip))?
        .to_string();

    info!("Detected physical gateway: {gw} via {dev} (local IP: {local_ip})");
    Some((gw, dev, local_ip))
}

/// Check whether an interface name looks like a VPN interface.
pub fn is_vpn_interface(name: &str) -> bool {
    let prefixes = ["tun", "tap", "wg", "ppp", "nordlynx", "tailscale"];
    prefixes.iter().any(|p| name.starts_with(p))
}

/// Check whether an interface name looks like a virtual/container interface
/// (Docker, libvirt, etc.) that should be skipped for gateway detection.
pub fn is_virtual_interface(name: &str) -> bool {
    let prefixes = ["docker", "br-", "veth", "virbr", "lxc", "podman"];
    prefixes.iter().any(|p| name.starts_with(p))
}

/// Check whether an IP address is link-local (169.254.x.x).
fn is_link_local(ip: &str) -> bool {
    ip.starts_with("169.254.")
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
    matches!(dst, "default" | "0.0.0.0/0" | "0.0.0.0/1" | "128.0.0.0/1")
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

/// Returns true if the IP string is in a private/RFC-1918 range:
/// 10.0.0.0/8, 172.16.0.0/12, or 192.168.0.0/16.
fn is_private_dns_ip(ip: &str) -> bool {
    let parts: Vec<u8> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 4 {
        return false;
    }
    match parts[0] {
        10 => true,
        172 => parts[1] >= 16 && parts[1] <= 31,
        192 => parts[1] == 168,
        _ => false,
    }
}

/// Read resolv.conf content from a path and extract private nameserver IPs.
fn parse_resolv_conf(content: &str) -> Vec<String> {
    let mut servers = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("nameserver") {
            let ip = rest.trim();
            if is_private_dns_ip(ip) {
                let ip = ip.to_string();
                if !servers.contains(&ip) {
                    servers.push(ip);
                }
            }
        }
    }
    servers
}

/// Returns true if `ip` falls within the given CIDR (IPv4 only).
fn ip_in_cidr(ip: &str, cidr: &str) -> bool {
    let (prefix_str, len_str) = match cidr.split_once('/') {
        Some(p) => p,
        None => return false,
    };
    let prefix_len: u32 = match len_str.parse() {
        Ok(n) if n <= 32 => n,
        _ => return false,
    };
    let parse = |s: &str| -> Option<u32> {
        let parts: Vec<u8> = s.split('.').filter_map(|p| p.parse().ok()).collect();
        if parts.len() != 4 {
            return None;
        }
        Some(
            ((parts[0] as u32) << 24)
                | ((parts[1] as u32) << 16)
                | ((parts[2] as u32) << 8)
                | (parts[3] as u32),
        )
    };
    let ip_u32 = match parse(ip) {
        Some(v) => v,
        None => return false,
    };
    let prefix_u32 = match parse(prefix_str) {
        Some(v) => v,
        None => return false,
    };
    let mask: u32 = if prefix_len == 0 {
        0
    } else {
        !0u32 << (32 - prefix_len)
    };
    (ip_u32 & mask) == (prefix_u32 & mask)
}

/// Filter DNS server IPs, removing any that fall within the given bypass subnets.
///
/// DNS servers inside VPN-routed subnets cannot be queried correctly by xray: xray sends
/// DNS packets with `sendThrough=LOCAL_LAN_IP` as source, but the corporate VPN expects
/// traffic sourced from the VPN-assigned IP. The mismatch causes the DNS server to drop
/// queries, which xray treats as a ~4-second timeout — making every public DNS lookup
/// take 4 s × N corporate-DNS-servers before finally reaching 1.1.1.1. Only LAN-reachable
/// DNS servers (e.g. the home router, 192.168.1.1) respond quickly and are safe to include.
pub fn filter_dns_servers_by_subnet(
    dns_servers: &[String],
    bypass_subnets: &[String],
) -> Vec<String> {
    dns_servers
        .iter()
        .filter(|ip| !bypass_subnets.iter().any(|subnet| ip_in_cidr(ip, subnet)))
        .cloned()
        .collect()
}

/// Detect private (corporate VPN) DNS servers from system resolver config.
/// Tries `/run/systemd/resolve/resolv.conf` first, then falls back to `/etc/resolv.conf`.
/// Returns only RFC-1918 nameserver IPs (i.e. pushed by a corporate VPN).
pub fn detect_vpn_dns_servers() -> Vec<String> {
    let candidates = ["/run/systemd/resolve/resolv.conf", "/etc/resolv.conf"];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            let servers = parse_resolv_conf(&content);
            if !servers.is_empty() {
                return servers;
            }
        }
    }
    Vec::new()
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
        // RFC-1918 ranges always included when any VPN is active
        assert!(subnets.contains(&"10.0.0.0/8".to_string()));
        assert!(subnets.contains(&"172.16.0.0/12".to_string()));
        assert!(subnets.contains(&"192.168.0.0/16".to_string()));
        // VPN-specific subnets and server IP
        assert!(subnets.contains(&"10.8.0.0/24".to_string()));
        assert!(subnets.contains(&"172.20.0.0/16".to_string()));
        assert!(subnets.contains(&"185.100.50.25/32".to_string()));
        // "10.0.0.0/8" deduplicated (from wg0 subnets + RFC-1918)
        assert_eq!(subnets.len(), 6);
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
        // RFC-1918 ranges always included; "10.0.0.0/8" deduplicated from both VPNs + RFC-1918
        assert_eq!(subnets.len(), 3);
        assert!(subnets.contains(&"10.0.0.0/8".to_string()));
        assert!(subnets.contains(&"172.16.0.0/12".to_string()));
        assert!(subnets.contains(&"192.168.0.0/16".to_string()));
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
    fn test_is_virtual_interface() {
        assert!(is_virtual_interface("docker0"));
        assert!(is_virtual_interface("br-abc123"));
        assert!(is_virtual_interface("veth12345"));
        assert!(is_virtual_interface("virbr0"));
        assert!(is_virtual_interface("lxcbr0"));

        assert!(!is_virtual_interface("eth0"));
        assert!(!is_virtual_interface("wlp2s0"));
        assert!(!is_virtual_interface("enp3s0"));
        assert!(!is_virtual_interface("lo"));
        assert!(!is_virtual_interface("tun0"));
    }

    #[test]
    fn test_ip_in_cidr() {
        assert!(ip_in_cidr("10.6.4.36", "10.6.0.0/16"));
        assert!(ip_in_cidr("10.6.4.36", "10.0.0.0/8"));
        assert!(ip_in_cidr("10.6.255.255", "10.6.0.0/16"));
        assert!(ip_in_cidr("192.168.1.1", "192.168.1.0/24"));

        assert!(!ip_in_cidr("10.7.0.1", "10.6.0.0/16"));
        assert!(!ip_in_cidr("192.168.1.1", "10.6.0.0/16"));
        assert!(!ip_in_cidr("not-an-ip", "10.0.0.0/8"));
        assert!(!ip_in_cidr("10.6.4.36", "not-a-cidr"));
    }

    #[test]
    fn test_filter_dns_servers_by_subnet() {
        let dns = vec![
            "10.6.4.36".to_string(),
            "10.6.8.36".to_string(),
            "192.168.1.1".to_string(),
        ];
        let subnets = vec!["10.6.0.0/16".to_string(), "10.201.0.0/16".to_string()];
        let filtered = filter_dns_servers_by_subnet(&dns, &subnets);
        // Corporate DNS servers in VPN subnet removed; home router kept
        assert_eq!(filtered, vec!["192.168.1.1"]);
    }

    #[test]
    fn test_filter_dns_servers_no_subnets() {
        let dns = vec!["10.6.4.36".to_string(), "192.168.1.1".to_string()];
        let filtered = filter_dns_servers_by_subnet(&dns, &[]);
        assert_eq!(filtered, dns); // nothing filtered
    }

    #[test]
    fn test_filter_dns_servers_all_removed() {
        let dns = vec!["10.6.4.36".to_string()];
        let subnets = vec!["10.0.0.0/8".to_string()];
        let filtered = filter_dns_servers_by_subnet(&dns, &subnets);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_is_private_dns_ip() {
        assert!(is_private_dns_ip("10.8.0.1"));
        assert!(is_private_dns_ip("10.0.0.1"));
        assert!(is_private_dns_ip("10.255.255.255"));
        assert!(is_private_dns_ip("172.16.0.1"));
        assert!(is_private_dns_ip("172.31.255.255"));
        assert!(is_private_dns_ip("192.168.1.1"));
        assert!(is_private_dns_ip("192.168.0.1"));

        assert!(!is_private_dns_ip("1.1.1.1"));
        assert!(!is_private_dns_ip("8.8.8.8"));
        assert!(!is_private_dns_ip("172.15.0.1"));
        assert!(!is_private_dns_ip("172.32.0.1"));
        assert!(!is_private_dns_ip("not-an-ip"));
        assert!(!is_private_dns_ip(""));
    }

    #[test]
    fn test_parse_resolv_conf_private_only() {
        let content = "\
nameserver 10.8.0.1
nameserver 1.1.1.1
nameserver 8.8.8.8
";
        let servers = parse_resolv_conf(content);
        assert_eq!(servers, vec!["10.8.0.1"]);
    }

    #[test]
    fn test_parse_resolv_conf_no_private() {
        let content = "\
nameserver 1.1.1.1
nameserver 8.8.8.8
";
        let servers = parse_resolv_conf(content);
        assert!(servers.is_empty());
    }

    #[test]
    fn test_parse_resolv_conf_multiple_private() {
        let content = "\
nameserver 10.8.0.1
nameserver 192.168.1.1
nameserver 1.1.1.1
";
        let servers = parse_resolv_conf(content);
        assert_eq!(servers, vec!["10.8.0.1", "192.168.1.1"]);
    }

    #[test]
    fn test_parse_resolv_conf_deduplicates() {
        let content = "\
nameserver 10.8.0.1
nameserver 10.8.0.1
";
        let servers = parse_resolv_conf(content);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0], "10.8.0.1");
    }

    #[test]
    fn test_parse_resolv_conf_ignores_comments_and_other_lines() {
        let content = "\
# Generated by vpn
search example.com
domain example.com
nameserver 10.8.0.1
options ndots:5
";
        let servers = parse_resolv_conf(content);
        assert_eq!(servers, vec!["10.8.0.1"]);
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
