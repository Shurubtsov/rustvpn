use serde_json::{json, Value};

use crate::models::{AppError, ServerConfig};

pub const STATS_API_ADDR: &str = "127.0.0.1:10085";

pub fn generate_client_config(
    server: &ServerConfig,
    socks_port: u16,
    bypass_domains: &[String],
    bypass_subnets: &[String],
) -> Result<String, AppError> {
    let mut config: Value = json!({
        "log": {
            "loglevel": "info"
        },
        "dns": {
            "servers": [
                "1.1.1.1",
                "8.8.8.8"
            ]
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

    // Private IPs + detected VPN subnets → direct
    let mut direct_ips = vec![
        "127.0.0.0/8".to_string(),
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
        "::1/128".to_string(),
        "fc00::/7".to_string(),
    ];
    for subnet in bypass_subnets {
        let s = subnet.trim();
        if !s.is_empty() && !direct_ips.contains(&s.to_string()) {
            direct_ips.push(s.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RealitySettings;

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

        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
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

        // Verify DNS
        assert_eq!(config["dns"]["servers"][0], "1.1.1.1");
        assert_eq!(config["dns"]["servers"][1], "8.8.8.8");
    }

    #[test]
    fn test_config_custom_socks_port() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 1080, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        assert_eq!(config["inbounds"][0]["port"], 1080);
    }

    #[test]
    fn test_config_has_required_outbounds() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
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
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let stream = &config["outbounds"][0]["streamSettings"];
        assert_eq!(stream["network"], "tcp");
        assert_eq!(stream["security"], "reality");
        assert_eq!(stream["realitySettings"]["show"], false);
    }

    #[test]
    fn test_config_encryption_is_none() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let user = &config["outbounds"][0]["settings"]["vnext"][0]["users"][0];
        assert_eq!(user["encryption"], "none");
    }

    #[test]
    fn test_config_routing_rules() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        assert_eq!(config["routing"]["domainStrategy"], "IPIfNonMatch");
        let rules = config["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["outboundTag"], "direct");
        assert!(rules[0]["domain"].as_array().unwrap().contains(&Value::String("localhost".to_string())));
        assert_eq!(rules[1]["outboundTag"], "direct");
        let ips = rules[1]["ip"].as_array().unwrap();
        assert!(ips.contains(&Value::String("127.0.0.0/8".to_string())));
        assert!(ips.contains(&Value::String("10.0.0.0/8".to_string())));
        assert!(ips.contains(&Value::String("192.168.0.0/16".to_string())));
    }

    #[test]
    fn test_config_sniffing_enabled() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
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
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
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
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
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
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
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
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        assert!(
            config.get("stats").is_some(),
            "config must contain 'stats' key"
        );
    }

    #[test]
    fn test_config_has_api_section() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let api = &config["api"];
        assert_eq!(api["tag"], "api");
        assert_eq!(api["listen"], "127.0.0.1:10085");
        let services = api["services"].as_array().unwrap();
        assert!(services.contains(&Value::String("StatsService".to_string())));
    }

    #[test]
    fn test_config_with_vpn_bypass_subnets() {
        let server = ServerConfig::default();
        let bypass_subnets = vec!["10.8.0.0/24".to_string(), "172.20.0.0/16".to_string()];
        let config_str = generate_client_config(&server, 10808, &[], &bypass_subnets).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let rules = config["routing"]["rules"].as_array().unwrap();
        // Find the IP-based direct rule
        let ip_rule = rules.iter().find(|r| r.get("ip").is_some()).unwrap();
        let ips = ip_rule["ip"].as_array().unwrap();
        assert!(ips.contains(&Value::String("10.8.0.0/24".to_string())));
        assert!(ips.contains(&Value::String("172.20.0.0/16".to_string())));
        // Standard private IPs should still be present
        assert!(ips.contains(&Value::String("127.0.0.0/8".to_string())));
        assert!(ips.contains(&Value::String("192.168.0.0/16".to_string())));
    }

    #[test]
    fn test_config_has_stats_policy() {
        let server = ServerConfig::default();
        let config_str = generate_client_config(&server, 10808, &[], &[]).unwrap();
        let config: Value = serde_json::from_str(&config_str).unwrap();

        let system = &config["policy"]["system"];
        assert_eq!(system["statsOutboundUplink"], true);
        assert_eq!(system["statsOutboundDownlink"], true);
    }
}
