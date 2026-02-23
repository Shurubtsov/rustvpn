# /release Skill

Create a release build of RustVPN.

## Steps
1. Run tests: `cd src-tauri && cargo test`
2. Run lints: `cargo clippy -- -D warnings && cargo fmt --check`
3. Build: `pnpm tauri build`
4. Report: binary path, size, and any warnings
