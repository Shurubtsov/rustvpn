# /lint Skill

Run all linters for RustVPN.

## Steps
1. Rust: `cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings`
2. Frontend: `pnpm lint && pnpm format --check` (if configured)
3. Report any issues found
