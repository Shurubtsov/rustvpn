# RustVPN

Desktop VPN client using the **VLESS + REALITY** protocol — currently the most DPI-resistant tunneling method, achieving a ~99.5% bypass rate against Russian TSPU deep packet inspection. Built with Tauri v2, Svelte 5, and xray-core.

RustVPN mimics normal HTTPS traffic to sites like `microsoft.com`, making VPN connections invisible to traffic analysis systems.

## Features

- **One-click connect/disconnect** via xray-core sidecar
- **VLESS + REALITY** protocol with XTLS Vision and TLS 1.3
- **Server management** — add, edit, delete, import/export
- **`vless://` URI support** — share and import server configs
- **Real-time speed graph** — upload/download stats with 60-second history
- **System tray** — connect/disconnect from tray, hide-to-tray on close
- **Corporate VPN auto-detection** — detects OpenVPN/WireGuard/Tailscale interfaces, auto-bypasses their subnets
- **Auto-connect** on startup with last used server
- **Log viewer** — searchable xray process logs with level filtering
- **Dark/light theme** with persistent toggle
- **Cross-platform** — Linux, Windows, macOS

## Screenshots

> *Run `pnpm tauri dev` to see the app in action.*

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + Tauri v2 |
| Frontend | Svelte 5 + SvelteKit + Tailwind CSS 4 |
| VPN Engine | xray-core (bundled sidecar binary) |
| Protocol | VLESS + REALITY (XTLS Vision, TLS 1.3) |
| Platforms | Linux, Windows, macOS |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (stable, >= 1.77.2)
- [Node.js](https://nodejs.org/) >= 22
- [pnpm](https://pnpm.io/) >= 9
- Linux system dependencies:
  ```bash
  # Arch/Manjaro
  sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg patchelf

  # Ubuntu/Debian
  sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
  ```

### Setup

```bash
git clone https://github.com/Shurubtsov/rustvpn.git
cd rustvpn

# Install frontend dependencies
pnpm install

# Download xray-core binary for your platform
./scripts/download-xray.sh

# Run in development mode
pnpm tauri dev
```

### Build for Production

```bash
pnpm tauri build
```

The built app will be in `src-tauri/target/release/bundle/`.

## Development

```bash
pnpm tauri dev         # Full dev mode with hot reload
cargo test             # Run Rust tests (from src-tauri/)
cargo clippy           # Rust linting
cargo fmt              # Rust formatting
pnpm check             # Svelte/TypeScript type checking
pnpm lint              # Frontend linting
pnpm format            # Frontend formatting
```

### xray-core Binary

The xray-core sidecar binary is not included in the repository (~35MB per platform). Use the download script:

```bash
./scripts/download-xray.sh              # Latest bundled version (v26.2.6)
./scripts/download-xray.sh v25.1.30     # Specific version
```

The script auto-detects your OS and architecture, downloads from [XTLS/Xray-core releases](https://github.com/XTLS/Xray-core/releases), and places the binary with the correct Tauri sidecar name in `src-tauri/binaries/`.

Supported platforms:
- `xray-x86_64-unknown-linux-gnu`
- `xray-x86_64-pc-windows-msvc.exe`
- `xray-aarch64-apple-darwin`
- `xray-x86_64-apple-darwin`

## Project Structure

```
src-tauri/              Rust backend
  src/
    lib.rs              App builder, plugins, setup
    commands.rs         Tauri IPC command handlers
    xray.rs             XrayManager — sidecar lifecycle, stats, logs
    config.rs           xray JSON config generation
    models.rs           Data types (ServerConfig, SpeedStats, etc.)
    network.rs          Corporate VPN detection (ip route parsing)
    storage.rs          Persistence (servers.json, settings.json)
    tray.rs             System tray integration
    uri.rs              vless:// URI parsing and serialization
  binaries/             xray-core sidecar (gitignored)
  capabilities/         Tauri permissions (desktop + mobile)

src/                    Svelte 5 frontend
  lib/
    components/         9 UI components
    stores/             3 reactive stores (Svelte 5 runes)
    api/tauri.ts        invoke() wrappers for all IPC commands
    types/index.ts      TypeScript interfaces mirroring Rust
    utils/              Formatting and platform utilities
  routes/
    +page.svelte        Main dashboard
    logs/+page.svelte   Log viewer

scripts/                Build utilities
  download-xray.sh      Cross-platform xray binary downloader

docs/                   Documentation
  ARCHITECTURE.md       System design and data flow
  DEVELOPMENT.md        Setup guide
  API.md                IPC API reference
  XRAY_CONFIG.md        Protocol and server config
```

## Server Setup

RustVPN requires a VLESS+REALITY server. See [docs/XRAY_CONFIG.md](docs/XRAY_CONFIG.md) for full server setup instructions.

Quick summary — install xray-core on your VDS and configure it with VLESS+REALITY inbound on port 443. The client needs:

| Parameter | Example |
|-----------|---------|
| Server address | `45.151.233.107` |
| Port | `443` |
| UUID | (generated with `xray uuid`) |
| Public key | (generated with `xray x25519`) |
| Short ID | (generated with `openssl rand -hex 8`) |
| SNI | `www.microsoft.com` |
| Fingerprint | `chrome` |

Servers can be added via the UI form or by importing a `vless://` URI.

## vless:// URI Format

```
vless://UUID@HOST:PORT?encryption=none&flow=xtls-rprx-vision&type=tcp&security=reality&sni=SNI&fp=chrome&pbk=PUBLIC_KEY&sid=SHORT_ID#NAME
```

## Architecture

The app runs xray-core as a Tauri sidecar process. When connected, xray-core creates a local SOCKS5 proxy on `127.0.0.1:10808` and HTTP proxy on `127.0.0.1:10809`, tunneling traffic through the VLESS+REALITY protocol to the remote server.

```
┌─────────────┐     IPC      ┌──────────────┐   sidecar   ┌────────────┐
│  Svelte UI  │ ──invoke()──→│  Rust/Tauri   │ ──spawn()──→│  xray-core │
│  (WebView)  │ ←──events────│  Backend      │ ←──stdout───│  (VLESS)   │
└─────────────┘              └──────────────┘              └────────────┘
                                                                 │
                                                          REALITY tunnel
                                                                 │
                                                           ┌─────▼─────┐
                                                           │  VDS/VPN  │
                                                           │  Server   │
                                                           └───────────┘
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed design documentation.

## CI/CD

GitHub Actions builds on every push to `main` and on pull requests:

- **Linux** (ubuntu-latest) — installs WebKit/GTK deps
- **Windows** (windows-latest) — MSVC toolchain
- **macOS** (macos-latest) — Apple Silicon

Each build runs clippy, tests, type checking, and produces platform-specific artifacts.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — system design, data flow, state machine
- [Development](docs/DEVELOPMENT.md) — prerequisites, setup, project structure
- [API Reference](docs/API.md) — all IPC commands with signatures
- [xray Config](docs/XRAY_CONFIG.md) — REALITY protocol, server setup, URI format

## License

MIT
