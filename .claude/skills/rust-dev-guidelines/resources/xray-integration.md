# xray-core Integration

## Sidecar Binary
- Located at: `src-tauri/binaries/xray-{target_triple}`
- Spawned via `tauri_plugin_shell`
- Config written to temp file, passed via `-config` flag

## Config Generation Flow
1. `ServerProfile` → `config_gen::generate_config()` → `XrayConfig`
2. `XrayConfig` → `serde_json::to_string_pretty()` → JSON string
3. JSON written to temp file → path passed to xray sidecar

## Process Lifecycle
1. Generate config JSON from ServerProfile
2. Write to temp file
3. Spawn sidecar with `-config /path/to/config.json`
4. Monitor stdout/stderr for status
5. On disconnect: kill child process, clean up temp file

## Stats Collection
- xray API endpoint: `127.0.0.1:10085`
- Query via gRPC or CLI: `xray api statsquery --server=127.0.0.1:10085`
- Poll every 1 second, compute speed as delta/interval
