# Rust Development Guidelines

## Tauri Command Pattern
```rust
#[tauri::command]
async fn command_name(state: tauri::State<'_, AppState>, param: Type) -> Result<ReturnType, String> {
    state.inner().do_something(param).map_err(|e| e.to_string())
}
```

## Error Handling
Use `thiserror` for all error enums:
```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("xray process failed: {0}")]
    XrayProcess(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("storage error: {0}")]
    Storage(#[from] std::io::Error),
}
```

## Sidecar Management
Use `tauri_plugin_shell::ShellExt` to spawn xray-core:
```rust
let sidecar = app.shell().sidecar("xray").unwrap();
let (mut rx, child) = sidecar.args(&["-config", &config_path]).spawn().unwrap();
```

## Model Structs
All models crossing IPC boundary:
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerProfile {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub uuid: String,
    pub flow: String,
    pub public_key: String,
    pub short_id: String,
    pub sni: String,
    pub fingerprint: String,
}
```
