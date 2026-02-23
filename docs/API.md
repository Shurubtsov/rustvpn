# RustVPN — Tauri IPC API Reference

All commands are invoked from the frontend via `src/lib/api/tauri.ts` using `@tauri-apps/api/core`'s `invoke()`. On the Rust side, commands return `Result<T, String>` — on error, the `invoke()` call rejects with the error string.

## TypeScript Interfaces

Defined in `src/lib/types/index.ts`. These mirror the Rust structs in `src-tauri/src/models.rs`.

```typescript
export interface RealitySettings {
  public_key: string;   // X25519 public key (Base64url)
  short_id: string;     // Hex short ID (max 16 chars)
  server_name: string;  // TLS SNI domain (e.g. "www.microsoft.com")
  fingerprint: string;  // TLS fingerprint (e.g. "chrome")
}

export interface ServerConfig {
  id: string;               // UUID v4 (generated internally, not the VLESS user UUID)
  name: string;             // Display name (optional, defaults to address)
  address: string;          // Server IP or hostname
  port: number;             // Server port (1–65535)
  uuid: string;             // VLESS user UUID (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
  flow: string;             // XTLS flow (e.g. "xtls-rprx-vision")
  reality: RealitySettings;
}

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'disconnecting'
  | 'error';

export interface ConnectionInfo {
  status: ConnectionStatus;
  server_name: string | null;      // Display name of the connected server
  server_address: string | null;   // IP/host of the connected server
  connected_since: number | null;  // Unix timestamp (seconds) of connect time
  error_message: string | null;    // Set when status is 'error'
}
```

---

## Connection Commands

### `connect`

Validates the server config and starts the xray sidecar process.

**Rust signature:**
```rust
pub fn connect(app: AppHandle<R>, manager: State<'_, XrayManager>, server_config: ServerConfig) -> Result<(), String>
```

**TypeScript wrapper:**
```typescript
export async function connect(config: ServerConfig): Promise<void>
// invoke('connect', { serverConfig: config })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `serverConfig` | `ServerConfig` | Full server configuration to connect to |

**Returns:** `Promise<void>`

**Error cases:**
- `"Server address must not be empty"` — address field is blank or whitespace
- `"Server port must be greater than 0"` — port is 0
- `"UUID must match format xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (hex characters)"` — invalid UUID
- `"Reality public_key must not be empty"` — public_key is blank
- `"Reality short_id must not be empty"` — short_id is blank
- `"Already connected or connecting"` — xray is already running
- `"Failed to get app data dir: ..."` — OS path resolution failure
- `"Failed to create sidecar command: ..."` — xray binary not found in bundles
- `"Failed to spawn xray: ..."` — OS process spawn failure

**Behavior:** Sets status to `connecting`, generates the xray JSON config, writes it to disk, and spawns xray. Status transitions to `connected` asynchronously when xray logs `"started"` to stderr.

---

### `disconnect`

Kills the xray process and cleans up the config file.

**Rust signature:**
```rust
pub fn disconnect(manager: State<'_, XrayManager>) -> Result<(), String>
```

**TypeScript wrapper:**
```typescript
export async function disconnect(): Promise<void>
// invoke('disconnect')
```

**Returns:** `Promise<void>`

**Error cases:**
- `"Failed to kill xray: ..."` — OS-level kill failure (rare)

**Behavior:** Sets status to `disconnecting`, sends SIGKILL to xray child process, deletes the temp config file, sets status to `disconnected`.

---

### `get_status`

Returns the current connection status as a string.

**Rust signature:**
```rust
pub fn get_status(manager: State<'_, XrayManager>) -> Result<ConnectionStatus, String>
```

**TypeScript wrapper:**
```typescript
export async function getStatus(): Promise<string>
// invoke('get_status')
```

**Returns:** `Promise<string>` — one of: `"disconnected"`, `"connecting"`, `"connected"`, `"disconnecting"`, `"error"`

**Error cases:** None (infallible).

**Note:** The `connectionStore` uses `get_connection_info` (which includes more data) for polling rather than this simpler command.

---

### `get_connection_info`

Returns the full connection state including server name, address, uptime timestamp, and error message.

**Rust signature:**
```rust
pub fn get_connection_info(manager: State<'_, XrayManager>) -> Result<ConnectionInfo, String>
```

**TypeScript wrapper:**
```typescript
export async function getConnectionInfo(): Promise<ConnectionInfo>
// invoke('get_connection_info')
```

**Returns:** `Promise<ConnectionInfo>`

**Error cases:** None (infallible).

**Usage:** Called on page mount and then polled every 1 second by `connectionStore` while connected.

---

### `test_connection`

Checks whether the local SOCKS5 proxy port is open and accepting TCP connections.

**Rust signature:**
```rust
pub fn test_connection(manager: State<'_, XrayManager>) -> Result<bool, String>
```

**TypeScript wrapper:** Not currently exposed in `tauri.ts` (available for future use).

**Returns:** `Promise<boolean>` — `true` if TCP connect to `127.0.0.1:10808` succeeds within 3 seconds, `false` otherwise.

**Error cases:** None (returns `false` on failure rather than throwing).

---

### `get_socks_port`

Returns the SOCKS5 port xray listens on.

**Rust signature:**
```rust
pub fn get_socks_port(manager: State<'_, XrayManager>) -> Result<u16, String>
```

**TypeScript wrapper:** Not currently exposed in `tauri.ts` (available for future use).

**Returns:** `Promise<number>` — always `10808` in the current implementation.

**Error cases:** None (infallible).

---

### `validate_config`

Validates a `ServerConfig` without starting a connection. Useful for form validation before saving.

**Rust signature:**
```rust
pub fn validate_config(server_config: ServerConfig) -> Result<(), String>
```

**TypeScript wrapper:** Not currently exposed in `tauri.ts` (available for future use; frontend does its own validation in `ServerForm.svelte`).

**Returns:** `Promise<void>`

**Error cases:** Same validation errors as `connect` (address empty, port zero, invalid UUID, empty public_key, empty short_id).

---

## Server CRUD Commands

### `get_servers`

Loads the full server list from the persisted `servers.json` file.

**Rust signature:**
```rust
pub fn get_servers(app: AppHandle<R>) -> Result<Vec<ServerConfig>, String>
```

**TypeScript wrapper:**
```typescript
export async function getServers(): Promise<ServerConfig[]>
// invoke('get_servers')
```

**Returns:** `Promise<ServerConfig[]>` — empty array if no file exists yet.

**Error cases:**
- `"Failed to get app config dir: ..."` — OS path resolution failure
- `"..."` — JSON deserialization failure (corrupted `servers.json`)

---

### `add_server`

Appends a new server to the list. Always assigns a fresh UUID v4 as the server's internal `id`.

**Rust signature:**
```rust
pub fn add_server(app: AppHandle<R>, server_config: ServerConfig) -> Result<ServerConfig, String>
```

**TypeScript wrapper:**
```typescript
export async function addServer(config: ServerConfig): Promise<ServerConfig>
// invoke('add_server', { serverConfig: config })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `serverConfig` | `ServerConfig` | Server to add. The `id` field is ignored and replaced. |

**Returns:** `Promise<ServerConfig>` — the saved server with its new `id`.

**Error cases:**
- Storage read/write errors.

---

### `update_server`

Replaces an existing server entry identified by `server_config.id`.

**Rust signature:**
```rust
pub fn update_server(app: AppHandle<R>, server_config: ServerConfig) -> Result<(), String>
```

**TypeScript wrapper:**
```typescript
export async function updateServer(config: ServerConfig): Promise<void>
// invoke('update_server', { serverConfig: config })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `serverConfig` | `ServerConfig` | Updated server. Must include the correct `id`. |

**Returns:** `Promise<void>`

**Error cases:**
- `"Server with id <id> not found"` — no server with that ID exists.
- Storage read/write errors.

---

### `delete_server`

Removes a server by its internal ID.

**Rust signature:**
```rust
pub fn delete_server(app: AppHandle<R>, id: String) -> Result<(), String>
```

**TypeScript wrapper:**
```typescript
export async function deleteServer(id: string): Promise<void>
// invoke('delete_server', { id })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `string` | Internal UUID of the server to delete |

**Returns:** `Promise<void>`

**Error cases:**
- `"Server with id <id> not found"` — no server with that ID exists.
- Storage read/write errors.

---

## Import / Export Commands

### `export_servers`

Serializes the full server list to a pretty-printed JSON string.

**Rust signature:**
```rust
pub fn export_servers(app: AppHandle<R>) -> Result<String, String>
```

**TypeScript wrapper:**
```typescript
export async function exportServers(): Promise<string>
// invoke('export_servers')
```

**Returns:** `Promise<string>` — pretty-printed JSON array of `ServerConfig` objects.

**Error cases:** Storage read errors; JSON serialization errors (should not occur in practice).

**Usage in UI:** `ImportExportBar` calls this, then uses `tauri-plugin-fs` to write the string to a user-chosen file path via the save dialog.

---

### `import_servers`

Parses a JSON array of server configs and appends them to the existing list. All imported servers receive fresh IDs.

**Rust signature:**
```rust
pub fn import_servers(app: AppHandle<R>, json: String) -> Result<Vec<ServerConfig>, String>
```

**TypeScript wrapper:**
```typescript
export async function importServers(json: string): Promise<ServerConfig[]>
// invoke('import_servers', { json })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `json` | `string` | JSON array string of `ServerConfig` objects |

**Returns:** `Promise<ServerConfig[]>` — the newly added servers with their assigned IDs.

**Error cases:**
- `"Invalid JSON: ..."` — input is not valid JSON or does not match `Vec<ServerConfig>` shape.
- Storage read/write errors.

---

### `parse_vless_uri_cmd`

Parses a `vless://` URI string into a `ServerConfig`. Does not save — use `add_server` afterward to persist.

**Rust signature:**
```rust
pub fn parse_vless_uri_cmd(uri: String) -> Result<ServerConfig, String>
```

**TypeScript wrapper:**
```typescript
export async function parseVlessUri(uri: string): Promise<ServerConfig>
// invoke('parse_vless_uri_cmd', { uri })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `uri` | `string` | A `vless://` URI string |

**Returns:** `Promise<ServerConfig>` — parsed server config with a fresh internal `id`.

**Error cases:**
- `"Configuration error: URI must start with vless://"` — wrong scheme
- `"Configuration error: Missing @ in vless URI"` — malformed authority
- `"Configuration error: Missing port in vless URI"` — no port
- `"Configuration error: Invalid port: <value>"` — port not a valid u16
- `"Configuration error: Missing closing ] for IPv6 address"` — malformed IPv6

**Query parameter mapping:**

| URI param | Field |
|-----------|-------|
| `flow` | `flow` |
| `sni` | `reality.server_name` |
| `fp` | `reality.fingerprint` (default: `"chrome"`) |
| `pbk` | `reality.public_key` |
| `sid` | `reality.short_id` |
| `#fragment` | `name` (URL-decoded) |

Unknown parameters (`encryption`, `type`, `security`, etc.) are silently ignored.

---

### `export_vless_uri`

Serializes a `ServerConfig` into a `vless://` URI string.

**Rust signature:**
```rust
pub fn export_vless_uri(server_config: ServerConfig) -> Result<String, String>
```

**TypeScript wrapper:**
```typescript
export async function exportVlessUri(config: ServerConfig): Promise<string>
// invoke('export_vless_uri', { serverConfig: config })
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `serverConfig` | `ServerConfig` | Server to serialize |

**Returns:** `Promise<string>` — a `vless://` URI.

**Error cases:** None (infallible).

**Output format:**
```
vless://UUID@ADDRESS:PORT?encryption=none&flow=FLOW&type=tcp&security=reality&sni=SNI&fp=FP&pbk=PBK&sid=SID#NAME
```

All string values are percent-encoded. The `name` fragment uses `%20` for spaces (uppercase hex in percent-encoded output).

---

## Error Handling Pattern

All Tauri commands return `Result<T, String>` in Rust. On the TypeScript side, a command failure causes `invoke()` to throw a string error. The stores and UI catch these errors:

```typescript
// In connectionStore
try {
  await connect(config);
} catch (err) {
  info = {
    ...info,
    status: 'error',
    error_message: err instanceof Error ? err.message : String(err)
  };
}
```

```typescript
// In +page.svelte
try {
  const imported = await servers.importFromJson(json);
  showToast(`Imported ${imported.length} server(s)`);
} catch (e) {
  showToast(`Import failed: ${e}`, 'error');
}
```

The UI surfaces errors as toast notifications (3-second auto-dismiss) or as a persistent error state in `StatusDisplay` when the connection itself is in error state.
