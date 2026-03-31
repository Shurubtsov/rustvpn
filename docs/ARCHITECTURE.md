# RustVPN — Architecture

## System Overview

RustVPN is a desktop VPN client that manages xray-core as a child process (sidecar). The Svelte frontend communicates with the Rust backend exclusively through Tauri's IPC bridge. The Rust backend spawns xray-core as a sidecar binary, which handles the actual tunnel to the remote VDS (Virtual Dedicated Server).

```mermaid
graph TD
    subgraph Desktop App [Tauri Desktop App]
        subgraph Frontend [Svelte Frontend - WebView]
            UI[+page.svelte]
            CS[connectionStore]
            SS[serversStore]
            API[src/lib/api/tauri.ts]
            UI --> CS
            UI --> SS
            CS --> API
            SS --> API
        end
        subgraph Backend [Rust Backend]
            IPC[Tauri IPC Bridge]
            CMD[commands.rs]
            XM[XrayManager]
            CFG[config.rs - generate_client_config]
            STG[storage.rs - servers.json]
            URI[uri.rs - VLESS URI parser]
            CMD --> XM
            CMD --> STG
            CMD --> URI
            XM --> CFG
        end
        API -->|invoke| IPC
        IPC --> CMD
    end

    subgraph Sidecar [xray-core process]
        XRAY[xray binary]
        SOCKS[SOCKS5 :10808]
        XRAY --> SOCKS
    end

    subgraph VDS [Remote Server]
        VLESS[VLESS+REALITY listener]
    end

    XM -->|spawn / kill| XRAY
    XM -->|writes xray_config.json| XRAY
    SOCKS -->|encrypted VLESS over TCP| VLESS
    App[System apps] -->|SOCKS5 proxy| SOCKS
```

## Android Architecture

On Android, the desktop sidecar approach is replaced by a VPN service with TUN-based packet routing.

```
User Apps
    ↓ (all traffic intercepted by Android VPN API)
┌──────────────────────────────┐
│  TUN Device (10.0.0.2/30)   │  ← created by RustVpnService via VpnService.Builder
└──────────────────────────────┘
    ↓ (raw IP packets via file descriptor)
┌──────────────────────────────┐
│  hev-socks5-tunnel (libhev)  │  ← reads TUN FD, converts to SOCKS5
│  Loaded via JNI dlopen       │
└──────────────────────────────┘
    ↓ (SOCKS5 TCP/UDP → 127.0.0.1:10808)
┌──────────────────────────────┐
│  xray-core (libxray)         │  ← SOCKS5 inbound → VLESS+REALITY outbound
│  Launched via Runtime.exec() │
└──────────────────────────────┘
    ↓ (encrypted VLESS over TCP)
┌──────────────────────────────┐
│  VPN Server (VDS)            │
└──────────────────────────────┘
```

### Key Android Components

| File | Responsibility |
|------|---------------|
| `tauri-plugin-vpn/android/.../RustVpnService.kt` | Android `VpnService` — creates TUN, launches xray and hev, manages lifecycle |
| `tauri-plugin-vpn/android/.../VpnPlugin.kt` | Tauri plugin bridge — handles VPN permission, starts/stops service, queries stats |
| `tauri-plugin-vpn/android/.../HevTunnel.kt` | Kotlin JNI wrapper — loads libhev.so via dlopen, runs tunnel in pthread |
| `tauri-plugin-vpn/android/.../cpp/hev_jni.c` | C JNI library — dlopen/dlsym wrapper for hev-socks5-tunnel shared library |
| `tauri-plugin-vpn/src/mobile.rs` | Rust plugin interface for Android (calls Kotlin via Tauri mobile plugin API) |
| `src-tauri/src/config.rs` | `modify_config_for_android()` — merges `sockopt.mark` (preserving existing fragment settings), removes HTTP inbound |

### Why JNI dlopen (not fork/exec)?

The pre-built `hev-socks5-tunnel` binary from GitHub releases is a Linux/glibc executable that **cannot run on Android** (which uses bionic libc). The previous fork/exec approach silently failed with exit code 127. Instead, hev-socks5-tunnel is compiled from source using NDK to produce an Android shared library (`libhev.so`). The JNI wrapper (`libhevjni.so`) loads it at runtime via `dlopen()`, resolves the `hev_socks5_tunnel_main_from_file` and `hev_socks5_tunnel_quit` symbols, and runs the tunnel in a pthread. The TUN file descriptor is passed via the hev YAML config `fd:` parameter.

### Routing Loop Prevention

`addRoute("0.0.0.0", 0)` routes all traffic through the TUN, including xray's own connection to the VPN server. On Android, `sockopt.mark` does **not** bypass VPN routing (unlike Linux iptables). Instead, `addDisallowedApplication(packageName)` excludes the app's UID from VPN routing. Since xray and hev run as child processes with the same UID, their network traffic bypasses the TUN. hev still reads the TUN FD directly (file descriptor access, not network routing), so it is unaffected.

### Android Binary Dependencies

| Binary | Config name | Source | Purpose |
|--------|-----------|--------|---------|
| `libxray.so` | xray-core | [XTLS/Xray-core releases](https://github.com/XTLS/Xray-core/releases) | VLESS+REALITY proxy engine |
| `libhev.so` | hev-socks5-tunnel | Built from source via NDK ([heiher/hev-socks5-tunnel](https://github.com/heiher/hev-socks5-tunnel)) | TUN → SOCKS5 packet converter |
| `libhevjni.so` | — | Compiled from `cpp/hev_jni.c` via NDK/CMake | JNI dlopen wrapper for libhev.so |

Binaries are placed in `tauri-plugin-vpn/android/src/main/jniLibs/arm64-v8a/` (xray, hev) and compiled automatically (hevjni).

## Module Structure

### Rust Backend (`src-tauri/src/`)

| File | Responsibility |
|------|---------------|
| `main.rs` | Entry point; calls `rustvpn_lib::run()` |
| `lib.rs` | Tauri builder setup: registers plugins, manages `XrayManager` state, registers all IPC commands |
| `models.rs` | Core data types: `ServerConfig`, `RealitySettings`, `ConnectionInfo`, `ConnectionStatus`, `DpiBypassSettings`, `AppSettings`, `AppError` |
| `commands.rs` | All `#[tauri::command]` handlers for connection and server CRUD |
| `xray.rs` | `XrayManager` struct — spawns/kills xray sidecar, monitors output, tracks connection state |
| `config.rs` | `generate_client_config()` — builds the xray JSON config from a `ServerConfig`; injects `sockopt.fragment` for DPI bypass when enabled |
| `network.rs` | `detect_vpn_routes()` — detects corporate VPN interfaces and subnets via `ip -j route show`; `collect_bypass_subnets()` flattens results for proxy/config consumption |
| `storage.rs` | Reads/writes `servers.json` in the OS app config directory |
| `uri.rs` | `parse_vless_uri()` and `to_vless_uri()` — VLESS URI serialization; also exposes `parse_vless_uri_cmd` and `export_vless_uri` as Tauri commands |

### Svelte Frontend (`src/`)

| Path | Responsibility |
|------|---------------|
| `src/routes/+layout.ts` | Sets `prerender = true`, `ssr = false` (static SPA) |
| `src/routes/+layout.svelte` | Root layout; injects CSS and favicon |
| `src/routes/+page.svelte` | Main page; orchestrates all stores and components |
| `src/lib/api/tauri.ts` | Thin wrappers around `invoke()` for every Tauri command |
| `src/lib/types/index.ts` | TypeScript interfaces mirroring Rust structs |
| `src/lib/stores/connection.svelte.ts` | Svelte 5 rune-based store for connection state and actions |
| `src/lib/stores/servers.svelte.ts` | Svelte 5 rune-based store for server list and selection |
| `src/lib/stores/settings.svelte.ts` | Svelte 5 rune-based store for app settings (auto-connect, DPI bypass) |
| `src/lib/components/ConnectButton.svelte` | Circular toggle button; reflects connection status via color |
| `src/lib/components/StatusDisplay.svelte` | Status indicator dot, connection timer, server info panel |
| `src/lib/components/ServerList.svelte` | Scrollable list of servers with selection, edit, delete |
| `src/lib/components/ServerForm.svelte` | Modal form for adding/editing a server manually |
| `src/lib/components/ImportExportBar.svelte` | Toolbar with Import/Export dropdowns (file + URI) |
| `src/lib/components/UriInputModal.svelte` | Modal text area for pasting a vless:// URI |
| `src/lib/utils/index.ts` | `cn()` helper — `clsx` + `tailwind-merge` |

## Data Flow: Connect/Disconnect Cycle

### Connect

```mermaid
sequenceDiagram
    participant UI as +page.svelte
    participant CS as connectionStore
    participant API as tauri.ts
    participant Rust as commands.rs
    participant XM as XrayManager
    participant CFG as config.rs
    participant XRAY as xray binary

    UI->>CS: connectVpn(selectedServer)
    CS->>API: connect(serverConfig)
    API->>Rust: invoke("connect", {serverConfig})
    Rust->>Rust: serverConfig.validate()
    Rust->>Rust: load settings (bypass_domains, dpi_bypass)
    Rust->>XM: manager.start(app, server, bypass_domains, dpi_bypass)
    XM->>XM: check not already connected
    XM->>XM: status = Connecting
    XM->>CFG: generate_client_config(server, 10808, ..., dpi_bypass)
    CFG-->>XM: xray JSON config string
    XM->>XM: write xray_config.json to app_data_dir
    XM->>XRAY: spawn sidecar("xray", ["run", "-c", config_file])
    XM->>XM: spawn background task monitoring stdout/stderr
    XRAY-->>XM: stderr contains "started"
    XM->>XM: status = Connected, connected_since = now()
    CS->>CS: refresh() + startPolling() every 1s
    CS->>API: getConnectionInfo() [polls]
    API->>Rust: invoke("get_connection_info")
    Rust-->>API: ConnectionInfo{status: connected, ...}
    API-->>CS: update info
    CS-->>UI: reactive update
```

### Disconnect

```mermaid
sequenceDiagram
    participant UI as +page.svelte
    participant CS as connectionStore
    participant API as tauri.ts
    participant Rust as commands.rs
    participant XM as XrayManager
    participant XRAY as xray binary

    UI->>CS: disconnectVpn()
    CS->>API: disconnect()
    API->>Rust: invoke("disconnect")
    Rust->>XM: manager.stop()
    XM->>XM: status = Disconnecting
    XM->>XRAY: child.kill()
    XM->>XM: remove xray_config.json
    XM->>XM: status = Disconnected
    Rust-->>API: Ok(())
    CS->>CS: refresh() + stopPolling()
    CS-->>UI: reactive update
```

## IPC Contract

All commands are registered in `src-tauri/src/lib.rs` via `tauri::generate_handler!`. The frontend calls them through `src/lib/api/tauri.ts`.

### Connection Commands

| Command name | Rust handler | Parameters | Return type |
|---|---|---|---|
| `connect` | `commands::connect` | `server_config: ServerConfig` | `Result<(), String>` |
| `disconnect` | `commands::disconnect` | _(none)_ | `Result<(), String>` |
| `get_status` | `commands::get_status` | _(none)_ | `Result<ConnectionStatus, String>` |
| `get_connection_info` | `commands::get_connection_info` | _(none)_ | `Result<ConnectionInfo, String>` |
| `test_connection` | `commands::test_connection` | _(none)_ | `Result<bool, String>` |
| `get_socks_port` | `commands::get_socks_port` | _(none)_ | `Result<u16, String>` |
| `validate_config` | `commands::validate_config` | `server_config: ServerConfig` | `Result<(), String>` |
| `detect_vpn_interfaces` | `commands::detect_vpn_interfaces` | _(none)_ | `Result<Vec<DetectedVpn>, String>` |

### Server CRUD Commands

| Command name | Rust handler | Parameters | Return type |
|---|---|---|---|
| `get_servers` | `commands::get_servers` | _(none)_ | `Result<Vec<ServerConfig>, String>` |
| `add_server` | `commands::add_server` | `server_config: ServerConfig` | `Result<ServerConfig, String>` |
| `update_server` | `commands::update_server` | `server_config: ServerConfig` | `Result<(), String>` |
| `delete_server` | `commands::delete_server` | `id: String` | `Result<(), String>` |

### Import/Export Commands

| Command name | Rust handler | Parameters | Return type |
|---|---|---|---|
| `export_servers` | `commands::export_servers` | _(none)_ | `Result<String, String>` (pretty JSON) |
| `import_servers` | `commands::import_servers` | `json: String` | `Result<Vec<ServerConfig>, String>` |
| `parse_vless_uri_cmd` | `uri::parse_vless_uri_cmd` | `uri: String` | `Result<ServerConfig, String>` |
| `export_vless_uri` | `uri::export_vless_uri` | `server_config: ServerConfig` | `Result<String, String>` |

## State Management

### Rust: `XrayManager` (`src-tauri/src/xray.rs`)

`XrayManager` is a Tauri managed state singleton (`Arc<Mutex<...>>` fields):

| Field | Type | Purpose |
|-------|------|---------|
| `child` | `Arc<Mutex<Option<CommandChild>>>` | Handle to the running xray process |
| `state` | `Arc<Mutex<ConnectionInfo>>` | Current connection status and metadata |
| `config_path` | `Arc<Mutex<Option<PathBuf>>>` | Path to the temp xray config file for cleanup |
| `detected_vpns` | `Arc<Mutex<Vec<DetectedVpn>>>` | Last detected corporate VPN interfaces and subnets |
| `bypass_subnets` | `Arc<Mutex<Vec<String>>>` | Flattened bypass subnets from VPN detection |

State transitions:

```
Disconnected → Connecting → Connected
Connected    → Disconnecting → Disconnected
(xray crash) → Error
```

A background async task (spawned via `tauri::async_runtime::spawn`) monitors xray's stderr. When it detects the word "started", it transitions state from `Connecting` to `Connected`. If xray exits unexpectedly and state is not `Disconnecting`, it sets state to `Error`.

### Svelte: `connectionStore` (`src/lib/stores/connection.svelte.ts`)

Built with Svelte 5 runes (`$state`, `$derived`). Polls `get_connection_info` every 1 second while connected.

| Property | Type | Description |
|----------|------|-------------|
| `info` | `ConnectionInfo` | Full connection info from backend |
| `isLoading` | `boolean` | True during connect/disconnect IPC calls |
| `isConnected` | `boolean` (derived) | `info.status === 'connected'` |
| `isTransitioning` | `boolean` (derived) | `connecting` or `disconnecting` |

Methods: `connectVpn(config)`, `disconnectVpn()`, `refresh()`, `startPolling()`, `stopPolling()`.

### Svelte: `serversStore` (`src/lib/stores/servers.svelte.ts`)

| Property | Type | Description |
|----------|------|-------------|
| `servers` | `ServerConfig[]` | Full list from backend storage |
| `selectedId` | `string \| null` | ID of the selected server |
| `selectedServer` | `ServerConfig \| null` (derived) | The selected server object |
| `selectedIndex` | `number` (derived) | Index of selected server |

Methods: `load()`, `addServer()`, `updateServer()`, `deleteServer()`, `selectServer(id)`, `selectServerByIndex(index)`, `importFromJson(json)`, `importFromUri(uri)`, `exportToJson()`, `exportToUri(server)`.

### Svelte: `settingsStore` (`src/lib/stores/settings.svelte.ts`)

| Property | Type | Description |
|----------|------|-------------|
| `settings` | `AppSettings` | Current app settings (auto-connect, bypass domains, DPI bypass) |
| `loaded` | `boolean` | Whether settings have been loaded from backend |

Methods: `load()`, `setAutoConnect(value)`, `setBypassDomains(domains)`, `setDpiBypass(enabled)`.
