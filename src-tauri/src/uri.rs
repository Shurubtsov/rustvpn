use crate::models::{AppError, RealitySettings, ServerConfig};

pub fn parse_vless_uri(uri: &str) -> Result<ServerConfig, AppError> {
    // Format: vless://UUID@ADDRESS:PORT?params#NAME
    let uri = uri.trim();

    if !uri.starts_with("vless://") {
        return Err(AppError::Config("URI must start with vless://".to_string()));
    }

    let rest = &uri[8..]; // after "vless://"

    // Split off fragment (#NAME)
    let (rest, name) = match rest.rfind('#') {
        Some(pos) => (&rest[..pos], url_decode(&rest[pos + 1..])),
        None => (rest, String::new()),
    };

    // Split off query string (?params)
    let (authority, query) = match rest.find('?') {
        Some(pos) => (&rest[..pos], &rest[pos + 1..]),
        None => (rest, ""),
    };

    // Parse UUID@ADDRESS:PORT
    let at_pos = authority
        .find('@')
        .ok_or_else(|| AppError::Config("Missing @ in vless URI".to_string()))?;
    let uuid = authority[..at_pos].to_string();
    let host_port = &authority[at_pos + 1..];

    // Handle IPv6 addresses in brackets
    let (address, port) = if host_port.starts_with('[') {
        let bracket_end = host_port
            .find(']')
            .ok_or_else(|| AppError::Config("Missing closing ] for IPv6 address".to_string()))?;
        let addr = host_port[1..bracket_end].to_string();
        let port_str = &host_port[bracket_end + 1..];
        let port_str = port_str
            .strip_prefix(':')
            .ok_or_else(|| AppError::Config("Missing port after IPv6 address".to_string()))?;
        let port: u16 = port_str
            .parse()
            .map_err(|_| AppError::Config(format!("Invalid port: {port_str}")))?;
        (addr, port)
    } else {
        let colon_pos = host_port
            .rfind(':')
            .ok_or_else(|| AppError::Config("Missing port in vless URI".to_string()))?;
        let addr = host_port[..colon_pos].to_string();
        let port_str = &host_port[colon_pos + 1..];
        let port: u16 = port_str
            .parse()
            .map_err(|_| AppError::Config(format!("Invalid port: {port_str}")))?;
        (addr, port)
    };

    // Parse query parameters
    let mut flow = String::new();
    let mut sni = String::new();
    let mut fingerprint = "chrome".to_string();
    let mut public_key = String::new();
    let mut short_id = String::new();

    for param in query.split('&') {
        if param.is_empty() {
            continue;
        }
        if let Some((key, value)) = param.split_once('=') {
            match key {
                "flow" => flow = url_decode(value),
                "sni" => sni = url_decode(value),
                "fp" => fingerprint = url_decode(value),
                "pbk" => public_key = url_decode(value),
                "sid" => short_id = url_decode(value),
                _ => {} // ignore unknown params (encryption, type, security, etc.)
            }
        }
    }

    Ok(ServerConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        address,
        port,
        uuid,
        flow,
        reality: RealitySettings {
            public_key,
            short_id,
            server_name: sni,
            fingerprint,
        },
    })
}

pub fn to_vless_uri(server: &ServerConfig) -> String {
    let name = url_encode(&server.name);
    format!(
        "vless://{}@{}:{}?encryption=none&flow={}&type=tcp&security=reality&sni={}&fp={}&pbk={}&sid={}#{}",
        server.uuid,
        server.address,
        server.port,
        url_encode(&server.flow),
        url_encode(&server.reality.server_name),
        url_encode(&server.reality.fingerprint),
        url_encode(&server.reality.public_key),
        url_encode(&server.reality.short_id),
        name,
    )
}

fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push('%');
                result.push(char::from(HEX_CHARS[(b >> 4) as usize]));
                result.push(char::from(HEX_CHARS[(b & 0x0f) as usize]));
            }
        }
    }
    result
}

const HEX_CHARS: [u8; 16] = *b"0123456789ABCDEF";

fn url_decode(s: &str) -> String {
    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                result.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(b' ');
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[tauri::command]
pub fn parse_vless_uri_cmd(uri: String) -> Result<ServerConfig, String> {
    parse_vless_uri(&uri).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_vless_uri(server_config: ServerConfig) -> Result<String, String> {
    Ok(to_vless_uri(&server_config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_server() -> ServerConfig {
        ServerConfig {
            id: "test-uri-id".to_string(),
            name: "My Server".to_string(),
            address: "1.2.3.4".to_string(),
            port: 443,
            uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings {
                public_key: "kieJgZYLW9ZiKbGLpKnv4XyVo6_42inSONJrr-96tUU".to_string(),
                short_id: "d64736262cd50811".to_string(),
                server_name: "www.microsoft.com".to_string(),
                fingerprint: "chrome".to_string(),
            },
        }
    }

    #[test]
    fn parse_valid_uri() {
        let uri = "vless://aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee@1.2.3.4:443?encryption=none&flow=xtls-rprx-vision&type=tcp&security=reality&sni=www.microsoft.com&fp=chrome&pbk=kieJgZYLW9ZiKbGLpKnv4XyVo6_42inSONJrr-96tUU&sid=d64736262cd50811#My%20Server";
        let config = parse_vless_uri(uri).unwrap();

        assert_eq!(config.uuid, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert_eq!(config.address, "1.2.3.4");
        assert_eq!(config.port, 443);
        assert_eq!(config.flow, "xtls-rprx-vision");
        assert_eq!(config.name, "My Server");
        assert_eq!(config.reality.server_name, "www.microsoft.com");
        assert_eq!(config.reality.fingerprint, "chrome");
        assert_eq!(
            config.reality.public_key,
            "kieJgZYLW9ZiKbGLpKnv4XyVo6_42inSONJrr-96tUU"
        );
        assert_eq!(config.reality.short_id, "d64736262cd50811");
        // Should have a generated UUID id
        assert!(!config.id.is_empty());
    }

    #[test]
    fn parse_uri_no_fragment() {
        let uri = "vless://aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee@1.2.3.4:443?flow=xtls-rprx-vision&sni=example.com&pbk=key&sid=id";
        let config = parse_vless_uri(uri).unwrap();
        assert_eq!(config.name, "");
        assert_eq!(config.address, "1.2.3.4");
    }

    #[test]
    fn parse_uri_invalid_scheme() {
        let result = parse_vless_uri("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn parse_uri_missing_at() {
        let result = parse_vless_uri("vless://no-at-sign:443");
        assert!(result.is_err());
    }

    #[test]
    fn parse_uri_missing_port() {
        let result = parse_vless_uri("vless://uuid@1.2.3.4");
        assert!(result.is_err());
    }

    #[test]
    fn parse_uri_invalid_port() {
        let result = parse_vless_uri("vless://uuid@1.2.3.4:abc");
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip() {
        let server = sample_server();
        let uri = to_vless_uri(&server);
        let parsed = parse_vless_uri(&uri).unwrap();

        assert_eq!(parsed.uuid, server.uuid);
        assert_eq!(parsed.address, server.address);
        assert_eq!(parsed.port, server.port);
        assert_eq!(parsed.flow, server.flow);
        assert_eq!(parsed.name, server.name);
        assert_eq!(parsed.reality.public_key, server.reality.public_key);
        assert_eq!(parsed.reality.short_id, server.reality.short_id);
        assert_eq!(parsed.reality.server_name, server.reality.server_name);
        assert_eq!(parsed.reality.fingerprint, server.reality.fingerprint);
    }

    #[test]
    fn to_uri_format() {
        let server = sample_server();
        let uri = to_vless_uri(&server);
        assert!(uri.starts_with("vless://"));
        assert!(uri.contains("@1.2.3.4:443"));
        assert!(uri.contains("encryption=none"));
        assert!(uri.contains("security=reality"));
        assert!(uri.contains("#My%20Server"));
    }

    #[test]
    fn url_encode_special_chars() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a+b"), "a%2Bb");
        assert_eq!(url_encode("simple"), "simple");
    }

    #[test]
    fn url_decode_special_chars() {
        assert_eq!(url_decode("hello%20world"), "hello world");
        assert_eq!(url_decode("a%2Bb"), "a+b");
        assert_eq!(url_decode("simple"), "simple");
    }

    #[test]
    fn parse_uri_empty_uuid() {
        // Empty UUID before @ sign
        let result = parse_vless_uri("vless://@1.2.3.4:443?flow=xtls-rprx-vision");
        // Should parse (empty uuid is accepted by parser), but the uuid field will be empty
        // This tests that the parser handles it gracefully without panicking
        match result {
            Ok(config) => assert_eq!(config.uuid, ""),
            Err(_) => {} // also acceptable
        }
    }

    #[test]
    fn parse_uri_wrong_scheme() {
        // vmess:// should fail
        let result = parse_vless_uri("vmess://aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee@1.2.3.4:443");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("vless://"),
            "expected scheme hint in error: {err}"
        );
    }

    #[test]
    fn parse_uri_trojan_scheme() {
        // trojan:// should fail
        let result = parse_vless_uri("trojan://password@1.2.3.4:443");
        assert!(result.is_err());
    }

    #[test]
    fn parse_uri_empty_string() {
        let result = parse_vless_uri("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_uri_port_out_of_range() {
        // 65536 exceeds u16::MAX
        let result = parse_vless_uri("vless://aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee@1.2.3.4:65536");
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_special_chars_in_name() {
        let mut server = sample_server();
        server.name = "My VPN Server (Fast)".to_string();
        let uri = to_vless_uri(&server);
        let parsed = parse_vless_uri(&uri).unwrap();
        assert_eq!(parsed.name, server.name);
    }

    #[test]
    fn roundtrip_all_fields_match() {
        // Explicit field-by-field verification for clarity
        let server = ServerConfig {
            id: "some-id".to_string(),
            name: "Test Server".to_string(),
            address: "192.168.1.100".to_string(),
            port: 8443,
            uuid: "12345678-1234-1234-1234-123456789abc".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            reality: RealitySettings {
                public_key: "ABC123xyz-public-key".to_string(),
                short_id: "abcd1234".to_string(),
                server_name: "www.example.org".to_string(),
                fingerprint: "firefox".to_string(),
            },
        };

        let uri = to_vless_uri(&server);
        let parsed = parse_vless_uri(&uri).unwrap();

        assert_eq!(parsed.uuid, server.uuid, "uuid mismatch");
        assert_eq!(parsed.address, server.address, "address mismatch");
        assert_eq!(parsed.port, server.port, "port mismatch");
        assert_eq!(parsed.flow, server.flow, "flow mismatch");
        assert_eq!(parsed.name, server.name, "name mismatch");
        assert_eq!(
            parsed.reality.public_key, server.reality.public_key,
            "public_key mismatch"
        );
        assert_eq!(
            parsed.reality.short_id, server.reality.short_id,
            "short_id mismatch"
        );
        assert_eq!(
            parsed.reality.server_name, server.reality.server_name,
            "server_name mismatch"
        );
        assert_eq!(
            parsed.reality.fingerprint, server.reality.fingerprint,
            "fingerprint mismatch"
        );
        // Parsed id will be a new UUID (not the original id), but must not be empty
        assert!(!parsed.id.is_empty(), "parsed id must not be empty");
    }

    #[test]
    fn parse_uri_unknown_params_ignored() {
        // Extra unknown params should not cause errors
        let uri = "vless://aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee@1.2.3.4:443?flow=xtls-rprx-vision&sni=example.com&pbk=key&sid=id&unknown_param=value&another=123";
        let config = parse_vless_uri(uri).unwrap();
        assert_eq!(config.address, "1.2.3.4");
        assert_eq!(config.flow, "xtls-rprx-vision");
    }

    #[test]
    fn to_uri_contains_required_vless_fields() {
        let server = sample_server();
        let uri = to_vless_uri(&server);

        assert!(uri.starts_with("vless://"), "must start with vless://");
        assert!(uri.contains(&server.uuid), "must contain UUID");
        assert!(uri.contains(&server.address), "must contain server address");
        assert!(
            uri.contains(&format!(":{}", server.port)),
            "must contain port"
        );
        assert!(uri.contains("pbk="), "must contain public key param");
        assert!(uri.contains("sid="), "must contain short_id param");
        assert!(uri.contains("sni="), "must contain SNI param");
        assert!(uri.contains("flow="), "must contain flow param");
        assert!(
            uri.contains("security=reality"),
            "must declare reality security"
        );
    }
}
