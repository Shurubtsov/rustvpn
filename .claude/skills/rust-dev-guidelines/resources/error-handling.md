# Error Handling

## Pattern
- Library code: `thiserror` enums with `#[from]` for automatic conversion
- Tauri commands: map to `String` at the boundary
- Never panic in production: use `?` operator everywhere

## Error Enum Template
```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("xray: {0}")]
    Xray(String),
    #[error("config: {0}")]
    Config(String),
    #[error("storage: {0}")]
    Storage(#[from] std::io::Error),
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
}
```
