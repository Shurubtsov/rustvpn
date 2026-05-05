# RustVPN — Developer Setup Guide

## Prerequisites

### Required tools

| Tool | Version | Install |
|------|---------|---------|
| Rust + Cargo | >= 1.77.2 | `curl https://sh.rustup.rs -sSf \| sh` |
| Node.js | >= 18 | via system package manager or nvm |
| pnpm | >= 9 | `npm install -g pnpm` |
| Tauri CLI (bundled) | 2.x | installed as dev dependency via pnpm |

### System dependencies (Linux)

Tauri requires several system libraries for the WebView and desktop integration:

```bash
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev patchelf polkit

# Arch / Manjaro
sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg polkit
```

Refer to the [official Tauri prerequisites](https://tauri.app/start/prerequisites/) for macOS and Windows.

### TUN mode helper (Linux only, optional)

For the full system-VPN experience on Linux, RustVPN runs `hev-socks5-tunnel` as root via a small privileged helper (`rustvpn-helper`) launched through `pkexec`. Install the helper and its polkit rule once:

```bash
sudo ./scripts/install-helper.sh
```

This places `/usr/local/sbin/rustvpn-helper` and the policy file from `polkit/`. Without it, the app falls back to system-proxy mode (works for most apps, but not every TCP/UDP source).

### xray-core binary (required at runtime)

The xray binary is not committed to version control. It must be placed at:

```
src-tauri/binaries/xray-<target-triple>
```

where `<target-triple>` is your platform identifier, for example:

- Linux x86_64: `xray-x86_64-unknown-linux-gnu`
- macOS Apple Silicon: `xray-aarch64-apple-darwin`
- Windows x86_64: `xray-x86_64-pc-windows-msvc.exe`

Download the appropriate release from [XTLS/Xray-core releases](https://github.com/XTLS/Xray-core/releases).

### Android-specific prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Android SDK | API 34 | via Android Studio or `sdkmanager` |
| Android NDK | 27.0.12077973 | `sdkmanager "ndk;27.0.12077973"` |
| Rust Android target | — | `rustup target add aarch64-linux-android` |

### Android binaries (required at runtime)

The Android build requires two native binaries placed in:

```
src-tauri/tauri-plugin-vpn/android/src/main/jniLibs/arm64-v8a/
├── libxray.so     # xray-core for Android ARM64
└── libhev.so      # hev-socks5-tunnel for Android ARM64
```

Download them using the helper script:

```bash
./scripts/download-android-binaries.sh
```

A JNI wrapper library (`libhevjni.so`) is compiled automatically by the NDK/CMake build — no manual download needed.

## Clone and Run

```bash
git clone <repo-url>
cd RustVPN

# Install frontend dependencies
pnpm install

# Start development mode (builds frontend + Rust backend, opens app window)
pnpm tauri dev
```

`pnpm tauri dev` runs `pnpm dev` (Vite dev server on http://localhost:5173) and the Tauri Rust backend concurrently. The app window connects to the Vite dev server for hot module reload.

## Available Commands

### Development

```bash
pnpm tauri dev          # Run full app in dev mode (hot reload)
pnpm dev                # Frontend dev server only (Vite, port 5173)
```

### Building

```bash
pnpm tauri build        # Production build: compiles Rust in release mode,
                        # bundles frontend, produces installer/AppImage
```

### Frontend

```bash
pnpm check              # svelte-check type checking
pnpm check:watch        # Type check in watch mode
```

### Rust

Run from `src-tauri/`:

```bash
cargo test              # Run all Rust unit tests
cargo clippy            # Lint (must be clean, no warnings)
cargo fmt               # Auto-format Rust code
```

## Project Structure

```
RustVPN/
├── src-tauri/                    # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs               # Binary entry point
│   │   ├── lib.rs                # Tauri builder, plugin registration, startup recovery, command handler
│   │   ├── models.rs             # ServerConfig, RealitySettings, ConnectionInfo, SpeedStats,
│   │   │                         #   LogEntry, AppSettings, DetectedVpn, AppError
│   │   ├── commands.rs           # All #[tauri::command] handlers
│   │   ├── xray.rs               # XrayManager: sidecar lifecycle, stats poller, log buffer
│   │   ├── config.rs             # generate_client_config() + modify_config_for_android()
│   │   ├── network.rs            # Corporate VPN detection (ip -j route show), DNS scrape
│   │   ├── proxy.rs              # System proxy enable/disable (Linux/Win/macOS) — desktop only
│   │   ├── tun.rs                # Linux TUN mode via rustvpn-helper / pkexec
│   │   ├── tray.rs               # System tray menu (desktop only)
│   │   ├── storage.rs            # Load/save servers.json + settings.json
│   │   └── uri.rs                # VLESS URI parse and serialize
│   ├── tauri-plugin-vpn/         # Custom plugin for Android VpnService (see Android Build below)
│   ├── binaries/
│   │   └── xray-<triple>         # xray-core binary (gitignored)
│   ├── icons/                    # App icons for all platforms
│   ├── Cargo.toml                # Rust dependencies
│   └── tauri.conf.json           # Tauri configuration (window, bundle, sidecar)
│
├── scripts/                      # Helper installer + Android binary downloader
│   ├── install-helper.sh         # Installs rustvpn-helper for Linux TUN mode
│   ├── rustvpn-helper            # The privileged TUN helper itself
│   └── download-android-binaries.sh
├── polkit/                       # polkit rule for rustvpn-helper
│
├── src/                          # Svelte 5 + SvelteKit frontend
│   ├── routes/
│   │   ├── +layout.ts            # prerender=true, ssr=false
│   │   ├── +layout.svelte        # Root layout (CSS, favicon)
│   │   ├── +page.svelte          # Main page: composes all components
│   │   └── logs/+page.svelte     # Log viewer route
│   ├── lib/
│   │   ├── api/
│   │   │   └── tauri.ts          # invoke() wrappers for all Tauri commands
│   │   ├── types/
│   │   │   └── index.ts          # TypeScript interfaces (mirrors Rust structs)
│   │   ├── stores/
│   │   │   ├── connection.svelte.ts  # Connection state, polling, speed stats
│   │   │   ├── servers.svelte.ts     # Server CRUD + selection + import/export
│   │   │   └── settings.svelte.ts    # AppSettings with rollback-on-save-failure
│   │   ├── components/
│   │   │   ├── ConnectButton.svelte
│   │   │   ├── StatusDisplay.svelte
│   │   │   ├── ServerList.svelte
│   │   │   ├── ServerForm.svelte
│   │   │   ├── ImportExportBar.svelte
│   │   │   ├── UriInputModal.svelte
│   │   │   ├── SpeedGraph.svelte         # Upload/download sparkline
│   │   │   ├── LogViewer.svelte          # Tail of in-memory log buffer
│   │   │   ├── BackgroundModeModal.svelte # Mobile battery/auto-launch prompt
│   │   │   ├── ThemeToggle.svelte
│   │   │   └── ui/                       # shadcn-svelte primitives
│   │   ├── hooks/                # (reserved)
│   │   ├── assets/
│   │   │   └── favicon.svg
│   │   ├── utils/
│   │   │   └── index.ts          # cn() = clsx + tailwind-merge
│   │   └── index.ts              # Barrel export
│   ├── app.css                   # Tailwind CSS base styles + CSS variables
│   ├── app.d.ts                  # SvelteKit ambient types
│   └── app.html                  # HTML shell
│
├── docs/                         # Project documentation
├── .claude/                      # Claude Code agents/skills/hooks
├── creds/                        # Per-host VLESS credentials (gitignored)
├── release-assets/               # Per-platform release artifacts (gitignored)
├── Dockerfile.android            # Reproducible Android build environment
├── svelte.config.js              # SvelteKit adapter-static config
├── vite.config.ts                # Vite + Tailwind plugin config
├── tsconfig.json                 # TypeScript config
├── package.json                  # Frontend scripts and dependencies
└── pnpm-lock.yaml
```

## How the xray Sidecar Works

### Binary naming convention

Tauri's `tauri-plugin-shell` sidecar feature expects the binary to be named with the target triple suffix. At build time the CLI resolves your current target triple and looks for:

```
src-tauri/binaries/xray-<target-triple>[.exe on Windows]
```

This is configured in `tauri.conf.json`:

```json
"bundle": {
  "externalBin": ["binaries/xray"]
}
```

The `"binaries/xray"` entry is the base name; Tauri automatically appends the triple.

### Runtime lifecycle

1. `XrayManager::start()` calls `app.shell().sidecar("xray")` to get a managed sidecar handle.
2. The sidecar is launched with args `["run", "-c", "<path_to_config>"]`.
3. xray logs to stderr. The manager monitors stderr for the string `"started"` to detect successful startup.
4. `XrayManager::stop()` calls `child.kill()` and removes the temporary config file from `app_data_dir`.

### Config file location

The generated xray JSON config is written to:

```
<app_data_dir>/xray_config.json
```

On Linux this is typically `~/.local/share/com.rustvpn.app/xray_config.json`. The file is deleted on disconnect.

### Server list and settings storage

Two JSON files are persisted in the OS app config directory:

```
<app_config_dir>/servers.json   # Vec<ServerConfig>
<app_config_dir>/settings.json  # AppSettings (auto_connect, last_server_id, bypass_domains)
```

On Linux: `~/.config/com.rustvpn.app/`. On startup, `lib.rs` reads `settings.json` and, if `auto_connect` is true, immediately reconnects to `last_server_id` (unless an Android `VpnService` is already running, in which case it adopts that session).

### Hide-to-tray and auto-connect

Closing the main window does **not** quit the app — `lib.rs` intercepts `WindowEvent::CloseRequested`, calls `prevent_close()`, and hides the window. The system tray (configured in `tray.rs`) keeps the connection alive in the background. Use the tray's **Quit** entry to actually exit, or send `SIGINT` via the terminal during `pnpm tauri dev`.

## Android Build

```bash
# One-time setup
rustup target add aarch64-linux-android
sdkmanager "ndk;27.0.12077973"
pnpm tauri android init

# Download native binaries
./scripts/download-android-binaries.sh

# Build debug APK
NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973 pnpm tauri android build --apk
```

The Android build compiles:
- Rust backend for `aarch64-linux-android`
- Kotlin plugin code (VpnService, VpnPlugin, HevTunnel)
- C JNI library (`libhevjni.so`) via NDK/CMake

### Android plugin structure

```
src-tauri/tauri-plugin-vpn/
├── src/                          # Rust plugin interface
│   ├── lib.rs                    # Plugin registration
│   ├── mobile.rs                 # Android: calls Kotlin via PluginHandle
│   ├── desktop.rs                # Desktop: stubs (returns NotSupported)
│   └── commands.rs               # Tauri IPC commands
├── android/
│   ├── build.gradle.kts          # Android library build config (CMake/NDK)
│   └── src/main/
│       ├── AndroidManifest.xml   # Permissions + VpnService declaration
│       ├── java/com/rustvpn/vpn/
│       │   ├── RustVpnService.kt # VPN lifecycle: TUN, xray, hev
│       │   ├── VpnPlugin.kt     # Tauri plugin bridge, stats query
│       │   └── HevTunnel.kt     # JNI wrapper — loads libhev.so via dlopen
│       ├── cpp/
│       │   ├── hev_jni.c         # JNI dlopen wrapper for hev-socks5-tunnel
│       │   └── CMakeLists.txt    # CMake config for JNI library
│       └── jniLibs/arm64-v8a/   # Pre-built binaries (gitignored)
│           ├── libxray.so
│           └── libhev.so
└── Cargo.toml
```

## Tauri Plugins Used

| Plugin | Purpose |
|--------|---------|
| `tauri-plugin-shell` | Spawns xray sidecar process (desktop only) |
| `tauri-plugin-vpn` | Android VpnService management (custom in-tree plugin) |
| `tauri-plugin-dialog` | Open/save file dialogs for JSON import/export |
| `tauri-plugin-fs` | Read/write files for JSON import/export |
| `tauri-plugin-log` | Structured logging (debug builds only) |

The `tauri` crate itself is enabled with the `tray-icon` feature so `tray.rs` can register a system tray menu on desktop builds.

## Key Rust Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tauri` | 2.10.0 (feat. `tray-icon`) | Desktop/mobile app framework |
| `tauri-build` | 2.5.4 | Build-script support for Tauri |
| `serde` / `serde_json` | 1.0 | JSON serialization |
| `thiserror` | 2 | Ergonomic error types |
| `uuid` | 1 (v4 feature) | UUID generation for server IDs |
| `log` | 0.4 | Logging facade |

## Key Frontend Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `svelte` | 5.x | UI framework (runes API) |
| `@sveltejs/kit` | 2.x | App framework / routing |
| `tailwindcss` | 4.x | Utility-first CSS |
| `@tauri-apps/api` | 2.x | `invoke()` and Tauri JS API |
| `@tauri-apps/plugin-dialog` | 2.x | JS bindings for dialog plugin |
| `@tauri-apps/plugin-fs` | 2.x | JS bindings for fs plugin |
| `clsx` + `tailwind-merge` | latest | Class name utility (`cn()`) |
