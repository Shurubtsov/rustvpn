use serde_json::{json, Value};

use crate::models::{AppError, DpiBypassSettings, ServerConfig};

pub const STATS_API_ADDR: &str = "127.0.0.1:10085";

pub fn generate_client_config(
    server: &ServerConfig,
    socks_port: u16,
    bypass_domains: &[String],
    bypass_subnets: &[String],
    send_through: Option<&str>,
    vpn_dns_servers: &[String],
    dpi_bypass: &DpiBypassSettings,
) -> Result<String, AppError> {
    // In TUN mode, skip localhost DNS entirely. The system resolver calls getaddrinfo()
    // which goes through /etc/resolv.conf — corporate VPNs push their own DNS server
    // there, and in TUN mode that DNS traffic may be unroutable, causing a 30-second
    // hang per lookup that freezes every dispatch goroutine.
    //
    // If corporate VPN DNS servers were detected (private IPs from resolv.conf), inject
    // them first with expectIPs so xray queries them and accepts the result only if the
    // resolved IP falls within the corporate VPN's routed ranges. This covers both
    // RFC-1918 internal servers and public IPs routed through the corporate VPN (e.g.
    // gitlab-paygate.paywb.info → 185.62.201.181). Responses outside these ranges are
    // rejected and fall through to 1.1.1.1/8.8.8.8 for public resolution.
    //
    // DNS queries to corporate DNS (10.x.x.x) are routed via the "direct-vpn" outbound
    // (no sendThrough), so the kernel assigns the correct VPN-assigned source IP via
    // `ip rule to 10.0.0.0/8 lookup main`.
    //
    // In proxy mode, include localhost first for local/corporate hostname resolution.
    let dns_servers: Vec<Value> = if send_through.is_some() {
        let mut servers: Vec<Value> = if !vpn_dns_servers.is_empty() {
            // Build expectIPs from bypass_subnets — these include RFC-1918 ranges
            // plus any VPN-specific subnets detected from the routing table.
            let expect_ips: Vec<&str> = if bypass_subnets.is_empty() {
                vec!["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]
            } else {
                bypass_subnets.iter().map(|s| s.as_str()).collect()
            };
            vpn_dns_servers
                .iter()
                .map(|ip| json!({ "address": ip, "expectIPs": expect_ips }))
                .collect()
        } else {
            Vec::new()
        };
        servers.push(json!("1.1.1.1"));
        servers.push(json!("8.8.8.8"));
        servers
    } else {
        vec![json!("localhost"), json!("1.1.1.1"), json!("8.8.8.8")]
    };

    let mut config: Value = json!({
        "log": {
            "loglevel": "info"
        },
        "dns": {
            "servers": dns_servers
        },
        "stats": {},
        "api": {
            "tag": "api",
            "listen": STATS_API_ADDR,
            "services": ["StatsService"]
        },
        "policy": {
            "system": {
                "statsOutboundUplink": true,
                "statsOutboundDownlink": true
            }
        },
        "inbounds": [
            {
                "tag": "socks-in",
                "port": socks_port,
                "listen": "127.0.0.1",
                "protocol": "socks",
                "settings": {
                    "udp": true
                },
                "sniffing": {
                    "enabled": true,
                    "destOverride": ["http", "tls"]
                }
            },
            {
                "tag": "http-in",
                "port": socks_port + 1,
                "listen": "127.0.0.1",
                "protocol": "http",
                "sniffing": {
                    "enabled": true,
                    "destOverride": ["http", "tls"]
                }
            }
        ],
        "outbounds": [
            {
                "tag": "proxy",
                "protocol": "vless",
                "settings": {
                    "vnext": [
                        {
                            "address": server.address,
                            "port": server.port,
                            "users": [
                                {
                                    "id": server.uuid,
                                    "flow": server.flow,
                                    "encryption": "none"
                                }
                            ]
                        }
                    ]
                },
                "streamSettings": {
                    "network": "tcp",
                    "security": "reality",
                    "realitySettings": {
                        "show": false,
                        "fingerprint": server.reality.fingerprint,
                        "serverName": server.reality.server_name,
                        "publicKey": server.reality.public_key,
                        "shortId": server.reality.short_id
                    }
                }
            },
            {
                "tag": "direct",
                "protocol": "freedom"
            },
            {
                "tag": "block",
                "protocol": "blackhole"
            }
        ],
        "routing": {
            "domainStrategy": "IPIfNonMatch",
            "rules": []
        }
    });

    // Inject DPI bypass fragment into proxy outbound's streamSettings.sockopt
    if dpi_bypass.enabled {
        if let Some(outbounds) = config.get_mut("outbounds").and_then(|o| o.as_array_mut()) {
            for outbound in outbounds.iter_mut() {
                if outbound.get("tag").and_then(|t| t.as_str()) == Some("proxy") {
                    let sockopt = outbound
                        .as_object_mut()
                        .unwrap()
                        .entry("streamSettings")
                        .or_insert(json!({}))
                        .as_object_mut()
                        .unwrap()
                        .entry("sockopt")
                        .or_insert(json!({}))
                        .as_object_mut()
                        .unwrap();
                    sockopt.insert(
                        "fragment".to_string(),
                        json!({
                            "packets": dpi_bypass.packets,
                            "length": dpi_bypass.length,
                            "interval": dpi_bypass.interval
                        }),
                    );
                }
            }
        }
    }

    // On Linux with TUN mode, bind outbound connections to the physical interface IP
    // so that `ip rule add from <local_ip> lookup main` routes them through the
    // physical interface, bypassing the TUN default route and preventing loops.
    if let Some(local_ip) = send_through {
        if let Some(outbounds) = config.get_mut("outbounds").and_then(|o| o.as_array_mut()) {
            for outbound in outbounds.iter_mut() {
                let tag = outbound.get("tag").and_then(|t| t.as_str()).unwrap_or("");
                if tag == "proxy" || tag == "direct" {
                    outbound
                        .as_object_mut()
                        .unwrap()
                        .insert("sendThrough".to_string(), json!(local_ip));
                }
            }

            // Add a separate "direct-vpn" outbound WITHOUT sendThrough for corporate
            // VPN subnet traffic. When a corporate VPN is active, traffic to VPN-routed
            // subnets must use the kernel's default source IP (VPN-assigned) rather than
            // sendThrough's LAN IP — otherwise the corporate VPN server rejects packets
            // with the wrong source address. The kernel's `ip rule to SUBNET lookup main`
            // ensures these packets bypass the TUN and use the corporate VPN route.
            if !bypass_subnets.is_empty() {
                outbounds.push(json!({
                    "tag": "direct-vpn",
                    "protocol": "freedom"
                }));
            }
        }
    }

    // Build routing rules dynamically
    let rules = config["routing"]["rules"].as_array_mut().unwrap();

    // Bypass domains → direct (skip VPN tunnel)
    if !bypass_domains.is_empty() {
        let domains: Vec<Value> = bypass_domains
            .iter()
            .flat_map(|d| {
                let d = d.trim().to_lowercase();
                // Add both "domain:" (matches subdomains) and "full:" variants
                vec![
                    Value::String(format!("domain:{d}")),
                    Value::String(format!("full:{d}")),
                ]
            })
            .collect();
        rules.push(json!({
            "type": "field",
            "outboundTag": "direct",
            "domain": domains
        }));
    }

    // Local domains → direct
    rules.push(json!({
        "type": "field",
        "outboundTag": "direct",
        "domain": ["localhost"]
    }));

    // VPN bypass subnets → direct-vpn (no sendThrough) when in TUN mode.
    // This rule must come BEFORE the general direct IP rule so that corporate VPN
    // traffic uses the kernel-assigned source IP (VPN-assigned) instead of sendThrough's
    // LAN IP. In proxy-only mode (no sendThrough), there's no source IP conflict,
    // so VPN subnets go to the normal "direct" outbound.
    if send_through.is_some() && !bypass_subnets.is_empty() {
        let vpn_ips: Vec<Value> = bypass_subnets
            .iter()
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(Value::String(s.to_string()))
                }
            })
            .collect();
        if !vpn_ips.is_empty() {
            rules.push(json!({
                "type": "field",
                "outboundTag": "direct-vpn",
                "ip": vpn_ips
            }));
        }
    }

    // Private IPs + multicast → direct (with sendThrough in TUN mode)
    let mut direct_ips = vec![
        "127.0.0.0/8".to_string(),
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
        "224.0.0.0/4".to_string(), // IPv4 multicast (mDNS, SSDP, etc.) — never proxy
        "::1/128".to_string(),
        "fc00::/7".to_string(),
        "ff00::/8".to_string(), // IPv6 multicast
    ];

    // Defense-in-depth: route VPN server IP directly (alongside helper's ip route add)
    if !server.address.is_empty() {
        let server_cidr = if server.address.contains('/') {
            server.address.clone()
        } else {
            format!("{}/32", server.address)
        };
        if !direct_ips.contains(&server_cidr) {
            direct_ips.push(server_cidr);
        }
    }
    // In proxy-only mode (no sendThrough), add bypass subnets to the normal direct rule.
    // In TUN mode they're handled by the direct-vpn rule above.
    if send_through.is_none() {
        for subnet in bypass_subnets {
            let s = subnet.trim();
            if !s.is_empty() && !direct_ips.contains(&s.to_string()) {
                direct_ips.push(s.to_string());
            }
        }
    }
    let ip_values: Vec<Value> = direct_ips.into_iter().map(Value::String).collect();
    rules.push(json!({
        "type": "field",
        "outboundTag": "direct",
        "ip": ip_values
    }));

    serde_json::to_string_pretty(&config).map_err(AppError::from)
}

/// Modify xray config JSON for Android:
/// - Add sockopt mark=255 to proxy outbound (prevents traffic loop through TUN)
/// - Remove HTTP inbound (unnecessary with TUN)
#[cfg(mobile)]
pub fn modify_config_for_android(config_json: &str) -> Result<String, AppError> {
    let mut config: serde_json::Value =
        serde_json::from_str(config_json).map_err(AppError::from)?;

    // Add sockopt mark to proxy outbound (merge with existing sockopt to preserve fragment settings)
    if let Some(outbounds) = config.get_mut("outbounds").and_then(|o| o.as_array_mut()) {
        for outbound in outbounds.iter_mut() {
            if outbound.get("tag").and_then(|t| t.as_str()) == Some("proxy") {
                let sockopt = outbound
                    .as_object_mut()
                    .unwrap()
                    .entry("streamSettings")
                    .or_insert(serde_json::json!({}))
                    .as_object_mut()
                    .unwrap()
                    .entry("sockopt")
                    .or_insert(serde_json::json!({}))
                    .as_object_mut()
                    .unwrap();
                sockopt.insert("mark".to_string(), serde_json::json!(255));
            }
        }
    }

    // Remove HTTP inbound (keep only SOCKS)
    if let Some(inbounds) = config.get_mut("inbounds").and_then(|i| i.as_array_mut()) {
        inbounds.retain(|inbound| inbound.get("protocol").and_then(|p| p.as_str()) != Some("http"));
    }

    serde_json::to_string_pretty(&config).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DpiBypassSettings, RealitySettings};

    #[test]
    fn test_generate_config() {
        let server = ServerConfig {
            id: "test-config-id".to_string(),
            name: "Test Server".to_string(),
            address: "45.151.233.107".to_string(),
            port: 443,
            uuid: "b472a988-1cd7-4221-b76f-9cea35f2df2f".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings {
                public_key: "kieJgZYLW9ZiKbGLpKnv4XyVo6_42inSONJrr-96tUU".to_string(),
                short_id: "d64736262cd50811".to_string(),
                server_name: "www.microsoft.com".to_string(),
                fingerprint: "chrome".to_string(),
            },
        };

        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        // Verify inbound
        assert_eq!(config["inbounds"][0]["port"], 10808);
        assert_eq!(config["inbounds"][0]["listen"], "127.0.0.1");
        assert_eq!(config["inbounds"][0]["protocol"], "socks");

        // Verify outbound
        let vnext = &config["outbounds"][0]["settings"]["vnext"][0];
        assert_eq!(vnext["address"], "45.151.233.107");
        assert_eq!(vnext["port"], 443);
        assert_eq!(
            vnext["users"][0]["id"],
            "b472a988-1cd7-4221-b76f-9cea35f2df2f"
        );
        assert_eq!(vnext["users"][0]["flow"], "xtls-rprx-vision");

        // Verify REALITY settings
        let reality = &config["outbounds"][0]["streamSettings"]["realitySettings"];
        assert_eq!(
            reality["publicKey"],
            "kieJgZYLW9ZiKbGLpKnv4XyVo6_42inSONJrr-96tUU"
        );
        assert_eq!(reality["shortId"], "d64736262cd50811");
        assert_eq!(reality["serverName"], "www.microsoft.com");
        assert_eq!(reality["fingerprint"], "chrome");

        // Verify DNS (proxy-only mode includes external servers)
        let dns = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(dns[0], "localhost");
        assert_eq!(dns[1], "1.1.1.1");
        assert_eq!(dns[2], "8.8.8.8");
    }

    #[test]
    fn test_config_custom_socks_port() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            1080,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        assert_eq!(config["inbounds"][0]["port"], 1080);
    }

    #[test]
    fn test_config_has_required_outbounds() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 3);

        assert_eq!(outbounds[0]["tag"], "proxy");
        assert_eq!(outbounds[0]["protocol"], "vless");

        assert_eq!(outbounds[1]["tag"], "direct");
        assert_eq!(outbounds[1]["protocol"], "freedom");

        assert_eq!(outbounds[2]["tag"], "block");
        assert_eq!(outbounds[2]["protocol"], "blackhole");
    }

    #[test]
    fn test_config_reality_security() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let stream = &config["outbounds"][0]["streamSettings"];
        assert_eq!(stream["network"], "tcp");
        assert_eq!(stream["security"], "reality");
        assert_eq!(stream["realitySettings"]["show"], false);
    }

    #[test]
    fn test_config_encryption_is_none() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let user = &config["outbounds"][0]["settings"]["vnext"][0]["users"][0];
        assert_eq!(user["encryption"], "none");
    }

    #[test]
    fn test_config_routing_rules() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        assert_eq!(config["routing"]["domainStrategy"], "IPIfNonMatch");
        let rules = config["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["outboundTag"], "direct");
        assert!(rules[0]["domain"]
            .as_array()
            .unwrap()
            .contains(&Value::String("localhost".to_string())));
        assert_eq!(rules[1]["outboundTag"], "direct");
        let ips = rules[1]["ip"].as_array().unwrap();
        assert!(ips.contains(&Value::String("127.0.0.0/8".to_string())));
        assert!(ips.contains(&Value::String("10.0.0.0/8".to_string())));
        assert!(ips.contains(&Value::String("192.168.0.0/16".to_string())));
    }

    #[test]
    fn test_config_sniffing_enabled() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let sniffing = &config["inbounds"][0]["sniffing"];
        assert_eq!(sniffing["enabled"], true);
        let overrides = sniffing["destOverride"].as_array().unwrap();
        assert!(overrides.contains(&Value::String("http".to_string())));
        assert!(overrides.contains(&Value::String("tls".to_string())));
    }

    #[test]
    fn test_config_is_valid_json() {
        let server = ServerConfig {
            id: "test-valid-json-id".to_string(),
            name: "Prod".to_string(),
            address: "1.2.3.4".to_string(),
            port: 443,
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings {
                public_key: "pubkey".to_string(),
                short_id: "shortid".to_string(),
                server_name: "example.com".to_string(),
                fingerprint: "chrome".to_string(),
            },
        };
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let parsed: Result<Value, _> = serde_json::from_str(&config_str);
        assert!(
            parsed.is_ok(),
            "generate_client_config output is not valid JSON"
        );
    }

    #[test]
    fn test_config_server_address_uuid_port() {
        let server = ServerConfig {
            id: "test-addr-uuid-port-id".to_string(),
            name: "S".to_string(),
            address: "99.88.77.66".to_string(),
            port: 1234,
            uuid: "cafe0000-cafe-cafe-cafe-cafe00000000".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings::default(),
        };
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let vnext = &config["outbounds"][0]["settings"]["vnext"][0];
        assert_eq!(vnext["address"], "99.88.77.66");
        assert_eq!(vnext["port"], 1234);
        assert_eq!(
            vnext["users"][0]["id"],
            "cafe0000-cafe-cafe-cafe-cafe00000000"
        );
    }

    #[test]
    fn test_config_reality_settings_all_fields_present() {
        let server = ServerConfig {
            id: "test-reality-all-fields-id".to_string(),
            name: "R".to_string(),
            address: "5.5.5.5".to_string(),
            port: 443,
            uuid: "uuid".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings {
                public_key: "mypublickey".to_string(),
                short_id: "myshortid".to_string(),
                server_name: "www.cloudflare.com".to_string(),
                fingerprint: "safari".to_string(),
            },
        };
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let reality = &config["outbounds"][0]["streamSettings"]["realitySettings"];
        assert_eq!(reality["publicKey"], "mypublickey");
        assert_eq!(reality["shortId"], "myshortid");
        assert_eq!(reality["serverName"], "www.cloudflare.com");
        assert_eq!(reality["fingerprint"], "safari");
    }

    #[test]
    fn test_config_has_stats_section() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        assert!(
            config.get("stats").is_some(),
            "config must contain 'stats' key"
        );
    }

    #[test]
    fn test_config_has_api_section() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let api = &config["api"];
        assert_eq!(api["tag"], "api");
        assert_eq!(api["listen"], "127.0.0.1:10085");
        let services = api["services"].as_array().unwrap();
        assert!(services.contains(&Value::String("StatsService".to_string())));
    }

    #[test]
    fn test_config_with_vpn_bypass_subnets_proxy_mode() {
        // In proxy-only mode (no sendThrough), bypass subnets go in the normal direct rule
        let server = ServerConfig::default();
        let bypass_subnets = vec!["10.8.0.0/24".to_string(), "172.20.0.0/16".to_string()];
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &bypass_subnets,
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let rules = config["routing"]["rules"].as_array().unwrap();
        let ip_rule = rules
            .iter()
            .find(|r| {
                r.get("ip").is_some()
                    && r.get("outboundTag").and_then(|t| t.as_str()) == Some("direct")
            })
            .unwrap();
        let ips = ip_rule["ip"].as_array().unwrap();
        assert!(ips.contains(&Value::String("10.8.0.0/24".to_string())));
        assert!(ips.contains(&Value::String("172.20.0.0/16".to_string())));
        assert!(ips.contains(&Value::String("127.0.0.0/8".to_string())));
        assert!(ips.contains(&Value::String("192.168.0.0/16".to_string())));
        // No direct-vpn outbound in proxy mode
        let outbounds = config["outbounds"].as_array().unwrap();
        assert!(!outbounds
            .iter()
            .any(|o| o.get("tag").and_then(|t| t.as_str()) == Some("direct-vpn")));
    }

    #[test]
    fn test_config_with_vpn_bypass_subnets_tun_mode() {
        // In TUN mode (sendThrough set), bypass subnets go to direct-vpn (no sendThrough)
        // to let the kernel assign the correct VPN-assigned source IP
        let server = ServerConfig::default();
        let bypass_subnets = vec!["10.8.0.0/24".to_string(), "185.62.200.0/22".to_string()];
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &bypass_subnets,
            Some("192.168.1.100"),
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        // Verify direct-vpn outbound exists without sendThrough
        let outbounds = config["outbounds"].as_array().unwrap();
        let direct_vpn = outbounds
            .iter()
            .find(|o| o.get("tag").and_then(|t| t.as_str()) == Some("direct-vpn"))
            .expect("direct-vpn outbound must exist in TUN mode with bypass subnets");
        assert!(
            direct_vpn.get("sendThrough").is_none(),
            "direct-vpn must NOT have sendThrough"
        );
        assert_eq!(direct_vpn["protocol"], "freedom");

        // Verify routing rule routes bypass subnets to direct-vpn
        let rules = config["routing"]["rules"].as_array().unwrap();
        let vpn_rule = rules
            .iter()
            .find(|r| r.get("outboundTag").and_then(|t| t.as_str()) == Some("direct-vpn"))
            .expect("direct-vpn routing rule must exist");
        let vpn_ips = vpn_rule["ip"].as_array().unwrap();
        assert!(vpn_ips.contains(&Value::String("10.8.0.0/24".to_string())));
        assert!(vpn_ips.contains(&Value::String("185.62.200.0/22".to_string())));

        // Verify the main direct IP rule does NOT contain bypass subnets
        let direct_rule = rules
            .iter()
            .find(|r| {
                r.get("ip").is_some()
                    && r.get("outboundTag").and_then(|t| t.as_str()) == Some("direct")
            })
            .unwrap();
        let direct_ips = direct_rule["ip"].as_array().unwrap();
        assert!(!direct_ips.contains(&Value::String("10.8.0.0/24".to_string())));
        assert!(!direct_ips.contains(&Value::String("185.62.200.0/22".to_string())));
        // But standard private IPs are still there
        assert!(direct_ips.contains(&Value::String("127.0.0.0/8".to_string())));
        assert!(direct_ips.contains(&Value::String("10.0.0.0/8".to_string())));
    }

    #[test]
    fn test_config_tun_mode_dns_no_localhost() {
        // In TUN mode, localhost DNS calls getaddrinfo() which uses the system resolver.
        // Corporate VPNs push their own DNS server into /etc/resolv.conf, and in TUN mode
        // that path may be broken — causing a 30s hang per lookup. Skip localhost entirely.
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            Some("192.168.1.100"),
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let dns = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(
            dns.len(),
            2,
            "TUN mode with no VPN DNS must only include 1.1.1.1 and 8.8.8.8"
        );
        assert_eq!(dns[0], "1.1.1.1");
        assert_eq!(dns[1], "8.8.8.8");
    }

    #[test]
    fn test_config_tun_mode_with_vpn_dns_servers() {
        // When corporate VPN DNS is detected, inject it first with expectIPs from
        // bypass_subnets so xray accepts DNS responses for IPs within VPN-routed ranges
        // (both private and public corporate IPs). Responses outside bypass_subnets are
        // rejected and fall through to 1.1.1.1/8.8.8.8.
        let server = ServerConfig::default();
        let vpn_dns = vec!["10.8.0.1".to_string()];
        let bypass_subnets = vec!["10.0.0.0/8".to_string(), "185.62.200.0/22".to_string()];
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &bypass_subnets,
            Some("192.168.1.100"),
            &vpn_dns,
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let dns = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(dns.len(), 3);
        assert_eq!(dns[0]["address"], "10.8.0.1");
        let expect_ips = dns[0]["expectIPs"].as_array().unwrap();
        // expectIPs should match bypass_subnets (VPN-routed ranges)
        assert!(expect_ips.iter().any(|v| v == "10.0.0.0/8"));
        assert!(expect_ips.iter().any(|v| v == "185.62.200.0/22"));
        assert_eq!(dns[1], "1.1.1.1");
        assert_eq!(dns[2], "8.8.8.8");
    }

    #[test]
    fn test_config_tun_mode_vpn_dns_fallback_private_cidrs() {
        // When vpn_dns_servers is set but bypass_subnets is empty, fall back to RFC-1918 CIDRs
        let server = ServerConfig::default();
        let vpn_dns = vec!["10.8.0.1".to_string()];
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            Some("192.168.1.100"),
            &vpn_dns,
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let dns = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(dns.len(), 3);
        assert_eq!(dns[0]["address"], "10.8.0.1");
        let expect_ips = dns[0]["expectIPs"].as_array().unwrap();
        assert!(expect_ips.iter().any(|v| v == "10.0.0.0/8"));
        assert!(expect_ips.iter().any(|v| v == "172.16.0.0/12"));
        assert!(expect_ips.iter().any(|v| v == "192.168.0.0/16"));
    }

    #[test]
    fn test_config_proxy_mode_dns_includes_localhost() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let dns = config["dns"]["servers"].as_array().unwrap();
        assert_eq!(dns[0], "localhost");
        assert_eq!(dns[1], "1.1.1.1");
        assert_eq!(dns[2], "8.8.8.8");
    }

    #[test]
    fn test_config_server_ip_in_direct_rules() {
        let server = ServerConfig {
            id: "test-server-ip-direct".to_string(),
            name: "S".to_string(),
            address: "45.151.233.107".to_string(),
            port: 443,
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings::default(),
        };
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let rules = config["routing"]["rules"].as_array().unwrap();
        let ip_rule = rules.iter().find(|r| r.get("ip").is_some()).unwrap();
        let ips = ip_rule["ip"].as_array().unwrap();
        assert!(
            ips.contains(&Value::String("45.151.233.107/32".to_string())),
            "Server IP should be in direct routing rules"
        );
    }

    #[test]
    fn test_config_send_through_sets_outbounds() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            Some("192.168.1.50"),
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        assert_eq!(outbounds[0]["sendThrough"], "192.168.1.50");
        assert_eq!(outbounds[1]["sendThrough"], "192.168.1.50");
        // block outbound should NOT have sendThrough
        assert!(outbounds[2].get("sendThrough").is_none());
    }

    #[test]
    fn test_config_has_stats_policy() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(
            &server,
            10808,
            &[],
            &[],
            None,
            &[],
            &DpiBypassSettings::default(),
        )
        .unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let system = &config["policy"]["system"];
        assert_eq!(system["statsOutboundUplink"], true);
        assert_eq!(system["statsOutboundDownlink"], true);
    }

    #[test]
    fn test_config_dpi_bypass_disabled_no_fragment() {
        let server = ServerConfig::default();
        let dpi = DpiBypassSettings {
            enabled: false,
            ..DpiBypassSettings::default()
        };
        let config_str = generate_client_config(&server, 10808, &[], &[], None, &[], &dpi).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        // No sockopt.fragment when DPI bypass is disabled
        let stream = &config["outbounds"][0]["streamSettings"];
        assert!(
            stream.get("sockopt").is_none(),
            "sockopt should not exist when DPI bypass is disabled"
        );
    }

    #[test]
    fn test_config_dpi_bypass_enabled_injects_fragment() {
        let server = ServerConfig::default();
        let dpi = DpiBypassSettings {
            enabled: true,
            packets: "tlshello".to_string(),
            length: "100-200".to_string(),
            interval: "10-20".to_string(),
        };
        let config_str = generate_client_config(&server, 10808, &[], &[], None, &[], &dpi).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let fragment = &config["outbounds"][0]["streamSettings"]["sockopt"]["fragment"];
        assert_eq!(fragment["packets"], "tlshello");
        assert_eq!(fragment["length"], "100-200");
        assert_eq!(fragment["interval"], "10-20");
    }

    #[test]
    fn test_config_dpi_bypass_only_on_proxy_outbound() {
        let server = ServerConfig::default();
        let dpi = DpiBypassSettings {
            enabled: true,
            packets: "tlshello".to_string(),
            length: "100-200".to_string(),
            interval: "10-20".to_string(),
        };
        let config_str = generate_client_config(&server, 10808, &[], &[], None, &[], &dpi).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let outbounds = config["outbounds"].as_array().unwrap();
        // direct and block outbounds should not have sockopt.fragment
        for ob in outbounds.iter().skip(1) {
            assert!(
                ob.get("streamSettings")
                    .and_then(|s| s.get("sockopt"))
                    .and_then(|s| s.get("fragment"))
                    .is_none(),
                "non-proxy outbound should not have fragment"
            );
        }
    }
}
