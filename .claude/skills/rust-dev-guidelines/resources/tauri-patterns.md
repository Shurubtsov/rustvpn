# Tauri v2 Patterns

## Plugin Registration
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
        commands::connection::connect,
        commands::connection::disconnect,
        commands::connection::get_status,
    ])
```

## State Management
```rust
struct AppState {
    xray_process: Mutex<Option<CommandChild>>,
    connection_status: Mutex<ConnectionStatus>,
}

// Register in setup:
app.manage(AppState::default());
```

## Event Emission
```rust
app.emit("connection-status", &status)?;
```
