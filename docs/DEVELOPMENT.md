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
    librsvg2-dev patchelf

# Arch / Manjaro
sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg
```

Refer to the [official Tauri prerequisites](https://tauri.app/start/prerequisites/) for macOS and Windows.

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
│   │   ├── lib.rs                # Tauri builder, plugin registration, command handler
│   │   ├── models.rs             # ServerConfig, RealitySettings, ConnectionInfo, AppError
│   │   ├── commands.rs           # All #[tauri::command] handlers
│   │   ├── xray.rs               # XrayManager: spawn/kill xray sidecar, state machine
│   │   ├── config.rs             # generate_client_config(): builds xray JSON config
│   │   ├── storage.rs            # Load/save servers.json from OS config dir
│   │   └── uri.rs                # VLESS URI parse and serialize
│   ├── binaries/
│   │   └── xray-<triple>         # xray-core binary (gitignored)
│   ├── icons/                    # App icons for all platforms
│   ├── Cargo.toml                # Rust dependencies
│   └── tauri.conf.json           # Tauri configuration (window, bundle, sidecar)
│
├── src/                          # Svelte 5 + SvelteKit frontend
│   ├── routes/
│   │   ├── +layout.ts            # prerender=true, ssr=false
│   │   ├── +layout.svelte        # Root layout (CSS, favicon)
│   │   └── +page.svelte          # Main page: composes all components
│   ├── lib/
│   │   ├── api/
│   │   │   └── tauri.ts          # invoke() wrappers for all Tauri commands
│   │   ├── types/
│   │   │   └── index.ts          # TypeScript interfaces (mirrors Rust structs)
│   │   ├── stores/
│   │   │   ├── connection.svelte.ts  # Connection state store (polling, connect/disconnect)
│   │   │   └── servers.svelte.ts     # Server list store (CRUD, selection, import/export)
│   │   ├── components/
│   │   │   ├── ConnectButton.svelte   # Round toggle button
│   │   │   ├── StatusDisplay.svelte   # Status dot, timer, server info
│   │   │   ├── ServerList.svelte      # Clickable server list
│   │   │   ├── ServerForm.svelte      # Add/edit server modal
│   │   │   ├── ImportExportBar.svelte # Import/Export toolbar
│   │   │   └── UriInputModal.svelte   # Paste vless:// URI modal
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
├── .claude/                      # Claude Code agent/skill configuration
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

### Server list storage

The server list is persisted as JSON in:

```
<app_config_dir>/servers.json
```

On Linux: `~/.config/com.rustvpn.app/servers.json`

## Tauri Plugins Used

| Plugin | Purpose |
|--------|---------|
| `tauri-plugin-shell` | Spawns xray sidecar process |
| `tauri-plugin-dialog` | Open/save file dialogs for JSON import/export |
| `tauri-plugin-fs` | Read/write files for JSON import/export |
| `tauri-plugin-log` | Structured logging in debug builds |

## Key Rust Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tauri` | 2.10.0 | Desktop app framework |
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
