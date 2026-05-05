# Rust Development Guidelines

## Tauri Command Pattern

Commands return `Result<T, String>` for IPC; convert internal `AppError` with `.map_err(|e| e.to_string())`. Take `State<'_, XrayManager>` and/or `AppHandle<R>` as needed.

```rust
#[tauri::command]
pub fn my_command<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, XrayManager>,
    arg: String,
) -> Result<MyResult, String> {
    manager.do_something(&app, &arg).map_err(|e| e.to_string())
}
```

Register the command in `src-tauri/src/lib.rs` inside the `tauri::generate_handler![...]` list.

## Platform Gating

Modules and fields that don't apply to every platform are `#[cfg(...)]`-gated:

- `#[cfg(desktop)]` — `proxy.rs`, `tray.rs`, `XrayManager.{child, config_path, bypass_domains, bypass_subnets}`, `tauri-plugin-shell`.
- `#[cfg(target_os = "linux")]` — `tun.rs`, stale-TUN cleanup in `lib.rs::run()`.
- `#[cfg(mobile)]` — `XrayManager::adopt_running_state()`, `config::modify_config_for_android()`, the `tauri-plugin-vpn` mobile path.

Mirror these gates anywhere new platform-specific code lands so non-target builds stay green.

## Error Handling

Use `thiserror` for error enums; never use `anyhow` in library code (CLAUDE.md rule). The shared error type lives in `models.rs`:

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Xray process error: {0}")]
    XrayProcess(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    // ...
}
```

## Sidecar Management (desktop)

`XrayManager` owns the sidecar handle. Spawn through `app.shell().sidecar("xray")`:

```rust
let (mut rx, child) = app.shell()
    .sidecar("xray")?
    .args(["run", "-c", config_path.to_string_lossy().as_ref()])
    .spawn()?;
```

Spawn an async task with `tauri::async_runtime::spawn` to drain `rx` (`CommandEvent::Stdout/Stderr/Terminated`) — log lines feed `XrayManager.logs` and the `"started"` marker drives the `Connecting → Connected` transition. Emit `"connection-status-changed"` on transitions so `tray.rs` can update its menu label.

## Linux TUN Mode

`tun.rs` does not call `iproute2` directly — it shells out to the privileged `rustvpn-helper` via `pkexec`. Pass the app PID and physical-interface IP so the helper can:

- watch the app and tear down `rvpn0` if the GUI dies,
- add `ip rule from <local_ip> lookup main` so xray's own outbound bypasses the TUN.

Never run `ip` / `iptables` from the GUI process itself.

## IPC-Boundary Models

All structs that cross the Tauri IPC boundary derive `Debug, Clone, Serialize, Deserialize` and use `snake_case` field names (Serde's default), matching the TypeScript interfaces in `src/lib/types/index.ts`. The canonical server type is split into `ServerConfig` + nested `RealitySettings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub id: String,           // internal UUID v4 (assigned on add/import)
    pub name: String,
    pub address: String,
    pub port: u16,
    pub uuid: String,         // VLESS user UUID
    pub flow: String,         // typically "xtls-rprx-vision"
    pub reality: RealitySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealitySettings {
    pub public_key: String,
    pub short_id: String,
    pub server_name: String,  // SNI
    pub fingerprint: String,
}
```

When you add a new IPC type, add the matching TS interface in `src/lib/types/index.ts` and a wrapper in `src/lib/api/tauri.ts`.

## Style Rules

- Keep `cargo clippy` clean with no warnings (CI gate).
- Run `cargo fmt` before committing.
- Don't `unwrap()` on user-controlled input — propagate `AppError` instead.
- Comments earn their place: explain *why* (a constraint, a workaround), not *what*.
