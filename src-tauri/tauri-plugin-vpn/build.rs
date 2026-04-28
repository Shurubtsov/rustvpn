const COMMANDS: &[&str] = &[
    "start_vpn",
    "stop_vpn",
    "get_vpn_status",
    "query_stats",
    "is_battery_optimization_ignored",
    "request_ignore_battery_optimization",
    "open_oem_background_settings",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
