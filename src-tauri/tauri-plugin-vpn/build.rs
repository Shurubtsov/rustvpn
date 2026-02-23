const COMMANDS: &[&str] = &["start_vpn", "stop_vpn", "get_vpn_status", "query_stats"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
