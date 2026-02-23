# Changelog

All notable changes to RustVPN are documented in this file.

## [0.2.0] — 2026-02-23

### Corporate VPN Auto-Detection
- New `network.rs` module that parses `ip -j route show` JSON output to detect active VPN interfaces
- Recognizes OpenVPN (`tun*`, `tap*`), WireGuard (`wg*`), PPP/L2TP (`ppp*`), NordVPN (`nordlynx*`), and Tailscale (`tailscale*`) interfaces
- Collects routed subnets per VPN interface, skipping default/catch-all routes
- Detects VPN server endpoint IPs from static host routes (`/32`) through physical interfaces
- Detected subnets are automatically added to gsettings ignore-hosts (system proxy bypass)
- Detected subnets are added to xray routing rules as direct-out IPs (defense-in-depth)
- New `detect_vpn_interfaces` IPC command for fresh detection from the UI
- New `DetectedVpn` type exposed to frontend: interface name, VPN type, subnets, server IP
- Collapsible "Corporate VPNs" section in the UI showing detected interfaces and subnets
- "Refresh" button in the UI to re-run detection on demand
- Detection runs automatically at connect time — no manual configuration needed
- 12 new unit tests covering route parsing, interface classification, subnet collection, and edge cases

## [0.1.0] — 2026-02-23

Initial release. Full-featured desktop VPN client with VLESS+REALITY protocol support.

### Core VPN Engine (Phase 1)
- xray-core sidecar integration with start/stop/restart lifecycle
- `ServerConfig` model with all VLESS+REALITY fields (UUID, public key, short ID, SNI, fingerprint)
- xray JSON config generation from server profiles
- SOCKS5 proxy on `127.0.0.1:10808`, HTTP proxy on `127.0.0.1:10809`
- Connection state machine: disconnected → connecting → connected → disconnecting
- Config validation before connect
- `vless://` URI parsing and serialization

### User Interface (Phase 2)
- Main dashboard with connect button, status display, and server list
- `ConnectButton` — circular toggle with color-coded connection states
- `StatusDisplay` — status indicator, connection timer, server info
- `ServerList` — scrollable list with select, edit, delete actions
- `ServerForm` — modal form for adding/editing servers with field validation
- Toast notifications for success/error feedback

### Server Management (Phase 3)
- Full CRUD operations (add, edit, delete servers)
- Persistence to `servers.json` in app config directory
- Export server list as JSON file via system file dialog
- Import servers from JSON file
- `vless://` URI import via paste modal
- Export individual servers as `vless://` URI for sharing

### Speed Monitoring (Phase 4)
- Real-time upload/download speed via xray Stats API
- Speed computation from traffic deltas (polled every second)
- `SpeedGraph` — canvas-based chart with 60-second rolling history
- Upload (blue) and download (green) lines with area fills
- Auto-scaling Y axis, grid lines, legend
- Speed formatting: B/s → KB/s → MB/s → GB/s
- Total traffic counters (upload and download)

### System Tray, Auto-connect, Logs (Phase 5)
- System tray icon with context menu: Show Window, Connect/Disconnect, Quit
- Tray menu text updates dynamically (Connect ↔ Disconnect)
- Left-click tray icon shows/focuses window
- Hide to tray on window close (instead of quitting)
- Auto-connect on startup with last used server
- `AppSettings` persistence (auto-connect toggle, last server ID)
- Log ring buffer (1000 entries max) capturing xray stdout/stderr
- `LogViewer` — searchable log display with level coloring
- Log levels auto-detected from xray output (error, warning, info)
- Dedicated `/logs` route

### Polish and Theming (Phase 6)
- Dark/light theme toggle with `localStorage` persistence
- Theme initialization on layout mount (prevents flash)
- Green glow-pulse animation on connect button when connected
- Animated toast notifications with slide-in/out and status icons
- `fadeIn` animation on speed stats
- Theme-compatible canvas colors for SpeedGraph

### Cross-Platform Support (Phase 7)
- xray-core binaries for Windows (x64), macOS (ARM + Intel), Linux (x64)
- `scripts/download-xray.sh` — automated binary downloader for any platform
- Conditional compilation: system tray gated with `#[cfg(desktop)]`
- Mobile capabilities file with restricted permissions (no shell)
- Desktop capabilities scoped to desktop platforms only
- Platform detection utilities in frontend (`isMobile()`, `isDesktop()`)
- Touch-friendly button sizes for mobile readiness
- `ResizeObserver`-based responsive SpeedGraph
- GitHub Actions CI/CD for Linux, Windows, macOS builds

### Infrastructure
- Multi-agent Claude Code setup (rust-dev, ui-dev, tester, analyst)
- Domain skills for Rust and Svelte development guidelines
- Command skills: `/build`, `/test`, `/lint`, `/release`
- Post-edit hooks: auto-clippy on Rust files, auto-prettier on frontend files
- Stop hook: runs `cargo test` on session end
- 54 Rust unit tests covering config generation, URI parsing, models, storage
- Full TypeScript strict mode with 171 checked files, 0 errors
