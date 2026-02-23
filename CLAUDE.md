# RustVPN — Project Instructions

## Overview
Desktop VPN client using VLESS+REALITY protocol via xray-core sidecar. Built with Tauri v2 (Rust backend) + Svelte 5 + SvelteKit (frontend).

## Tech Stack
- **Backend**: Rust + Tauri v2
- **Frontend**: Svelte 5 + SvelteKit + Tailwind CSS + shadcn-svelte
- **VPN Engine**: xray-core (bundled as Tauri sidecar)
- **Protocol**: VLESS + REALITY (XTLS Vision, TLS 1.3)

## Commands
- `pnpm tauri dev` — Run in development mode
- `pnpm tauri build` — Production build
- `cargo test` — Run Rust tests (from src-tauri/)
- `cargo clippy` — Rust linting (from src-tauri/)
- `cargo fmt` — Rust formatting (from src-tauri/)
- `pnpm check` — Svelte type checking
- `pnpm lint` — Frontend linting
- `pnpm format` — Frontend formatting

## Project Structure
- `src-tauri/` — Rust backend (Tauri commands, xray-core management, models, storage)
- `src/` — Svelte 5 frontend (components, stores, routes, API wrappers)
- `src-tauri/binaries/` — xray-core sidecar binary
- `.claude/` — Claude Code agents, skills, hooks

## Conventions
### Rust
- Use `thiserror` for error types, never `anyhow` in library code
- All structs that cross IPC boundary: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Commands return `Result<T, String>` for Tauri IPC
- Keep clippy clean with no warnings

### Frontend
- Use Svelte 5 runes (`$state`, `$derived`, `$effect`) — no legacy stores
- TypeScript strict mode
- All Tauri `invoke()` calls wrapped in `src/lib/api/tauri.ts`
- TypeScript interfaces mirror Rust structs in `src/lib/types/index.ts`
- Use shadcn-svelte components where possible

### General
- Never commit credentials or secrets
- Keep xray-core binary in .gitignore (large binary)

## Orchestration Rules
When implementing a feature that touches both backend and frontend:
1. Launch rust-dev and ui-dev agents IN PARALLEL (single message, multiple Task calls)
2. Launch analyst agent in BACKGROUND for documentation
3. WAIT for dev agents to complete
4. Launch tester agent to verify changes
5. Review test results before moving to next feature

When implementing backend-only changes:
1. Launch rust-dev agent
2. Launch analyst in background
3. After rust-dev completes → launch tester

Maximize parallelism: if two agents don't depend on each other's output, launch them simultaneously.
